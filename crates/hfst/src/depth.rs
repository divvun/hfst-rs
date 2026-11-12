//! How deep a compile may recurse, and the stack it runs on.
//!
//! The compilers recurse: the nfst parsers descend once per bracket and per
//! prefix operator, the evaluators once per operation of the tree a parser
//! builds, dropping that tree costs a frame per level, and the tropical
//! backend minimizes an acyclic transducer by recursing along its longest
//! path. A stack overflow aborts the process, which no caller can catch, so
//! each is held under a limit that keeps it well inside the stack a compile
//! runs on: [`COMPILE_STACK_SIZE`] natively, and on wasm32 what a JavaScript
//! engine gives a module, which nothing can raise. Each limit holds with
//! every other at its own limit too, the stack at most half used.
//!
//! The parsers are another crate's, so their input is measured before they
//! see it: a scan of a source's tokens fails once its brackets nest
//! deeper than [`MAX_NESTING_DEPTH`], its evaluation would descend deeper
//! than [`MAX_EXPRESSION_DEPTH`], or its tree is deeper than
//! [`MAX_TREE_DEPTH`]. What an evaluation adds on top, through definitions,
//! function calls and files read, is counted as it happens, a level at a
//! time.
//!
//! The limits were measured by running each construct nested deeper and
//! deeper on a thread of a given stack, natively, and in a wasm32 build of
//! the same cases under Node 26, whose V8 gives wasm 984 KiB of stack: in
//! release builds on macOS arm64. They hold for an optimized build: an unoptimized one's frames are
//! 4 to 17 times as large. Giella's regular expressions nest brackets at most
//! 5 deep, its twolc rules evaluate at most 103 operators deep (lang-smj's
//! phonology.twolc) and its chains run to 912 operators (lang-sme's
//! generated oov-tagclass.regex), so each limit sits well above them on
//! wasm32 too.

use std::cell::Cell;
use std::ops::Range;

/// The stack a compile is given natively: the `hfst` tools run on a thread
/// this large, and `hfst-embed` runs each compile and operation on one. A
/// 64-bit host only reserves it, so it costs what is used of it.
pub const COMPILE_STACK_SIZE: usize = 512 * 1024 * 1024;

// [spec:hfst:req:embed.no-panic]
/// The deepest brackets nest: `[`, `(`, `[.` and the parentheses of a
/// function call or a built-in such as `Cap(`. A level is a descent of a
/// parser and of an evaluator. Natively it costs 9.5 KiB of stack in XRE,
/// 5.5 KiB in pmatch and 1.6 KiB in twolc, so 4000 levels take 37 MiB. On
/// wasm32 it costs up to 1.9 KiB of V8's stack (XRE traps at 687 levels,
/// pmatch at 527) and up to 5.2 KiB of the module's own.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_NESTING_DEPTH: usize = 4000;
// [spec:hfst:req:embed.no-panic]
/// See the native value.
#[cfg(target_arch = "wasm32")]
pub const MAX_NESTING_DEPTH: usize = 100;

// [spec:hfst:req:embed.no-panic]
/// The deepest an evaluation descends: prefix and postfix operators such as
/// `~` and `.i` applied to one another, a chain of pmatch or twolc
/// operators, which their evaluators recurse down, and the definitions,
/// function calls and files an evaluation goes through. Natively a level
/// costs at most 1.9 KiB of stack (XRE; pmatch 1.2 KiB, twolc 1.5 KiB), so
/// 100,000 levels take 190 MiB. On wasm32 it costs up to 0.45 KiB of V8's
/// stack (XRE traps at 2187 levels) and up to 3.5 KiB of the module's.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_EXPRESSION_DEPTH: usize = 100_000;
// [spec:hfst:req:embed.no-panic]
/// See the native value.
#[cfg(target_arch = "wasm32")]
pub const MAX_EXPRESSION_DEPTH: usize = 500;

// [spec:hfst:req:embed.no-panic]
/// The deepest tree a parser may build. XRE evaluates a chain of operators,
/// such as a union of a word list, in a loop, so the tree of one is deep
/// where its evaluation is not, and only its drop recurses down it: natively
/// at 46 bytes of stack a level, so 1,000,000 levels take 46 MiB, and on
/// wasm32 at 95 bytes of V8's stack (traps at 10,351 levels).
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_TREE_DEPTH: usize = 1_000_000;
// [spec:hfst:req:embed.no-panic]
/// See the native value.
#[cfg(target_arch = "wasm32")]
pub const MAX_TREE_DEPTH: usize = 3000;

// [spec:hfst:req:embed.no-panic]
/// The longest path, in arcs, of an acyclic transducer the tropical backend
/// minimizes: its minimizer finds each state's height by recursing along the
/// path. Natively an arc costs 128 bytes of stack, so 500,000 take 61 MiB;
/// on wasm32 it costs 0.18 KiB of V8's stack (traps at 5615 arcs).
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_PATH_LENGTH: usize = 500_000;
// [spec:hfst:req:embed.no-panic]
/// See the native value.
#[cfg(target_arch = "wasm32")]
pub const MAX_PATH_LENGTH: usize = 1000;

// [spec:hfst:req:embed.no-panic]
/// Run `run` on the stack a compile is given: natively on a thread of
/// [`COMPILE_STACK_SIZE`], under the `tracing` dispatcher the caller is
/// under, and on wasm32, which has no threads, where it is. A panic in `run`
/// is resumed here. Should the thread not start, `run` runs where it is.
pub fn on_compile_stack<T: Send>(run: impl FnOnce() -> T + Send) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dispatch = tracing::dispatcher::get_default(|d| d.clone());
        // A thread that fails to start drops what it was to run, so `run`
        // waits here for whichever of the two takes it.
        let slot = std::sync::Mutex::new(Some(run));
        let take = || {
            slot.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
        };
        let ran = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(COMPILE_STACK_SIZE)
                .spawn_scoped(scope, || {
                    take().map(|run| tracing::dispatcher::with_default(&dispatch, run))
                })
                .ok()
                .map(|thread| thread.join())
        });
        match ran {
            Some(Ok(Some(value))) => value,
            Some(Err(panic)) => std::panic::resume_unwind(panic),
            Some(Ok(None)) | None => {
                take().expect("a compile thread that did not start left `run` behind")()
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    run()
}

// The stack is the thread's, so the count of how much of it the compiles on
// a thread hold is the thread's too. A compile nested in another, as a file
// a regex reads or a lexc file pmatch reads, adds to the same count.
thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// The levels of [`MAX_EXPRESSION_DEPTH`] the compiles on this thread hold.
pub(crate) fn depth() -> usize {
    DEPTH.with(Cell::get)
}

/// Levels of [`MAX_EXPRESSION_DEPTH`] held for as long as the value lives.
#[must_use]
pub(crate) struct Descent(usize);

impl Descent {
    /// `levels` more, or `None` when they would pass the limit.
    pub(crate) fn enter(levels: usize) -> Option<Descent> {
        DEPTH.with(|depth| {
            let deeper = depth.get().checked_add(levels)?;
            (deeper <= MAX_EXPRESSION_DEPTH).then(|| {
                depth.set(deeper);
                Descent(levels)
            })
        })
    }
}

impl Drop for Descent {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - self.0));
    }
}

/// Which limit a source passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// [`MAX_NESTING_DEPTH`].
    Nesting,
    /// [`MAX_EXPRESSION_DEPTH`].
    Expression,
    /// [`MAX_TREE_DEPTH`].
    Tree,
}

// [spec:hfst:req:embed.no-panic]
/// A source too deep to compile, and where in it the limit was passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooDeep {
    pub limit: Limit,
    pub span: Range<usize>,
}

impl std::fmt::Display for TooDeep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.limit {
            Limit::Nesting => write!(f, "brackets are nested more than {MAX_NESTING_DEPTH} deep"),
            Limit::Expression => {
                write!(
                    f,
                    "operators are nested more than {MAX_EXPRESSION_DEPTH} deep"
                )
            }
            Limit::Tree => write!(
                f,
                "the expression chains more than {MAX_TREE_DEPTH} operators"
            ),
        }
    }
}

impl std::error::Error for TooDeep {}

/// The error an evaluation fails with past [`MAX_EXPRESSION_DEPTH`].
pub(crate) fn too_deep() -> crate::error::Error {
    crate::err!(
        Hfst,
        TooDeep {
            limit: Limit::Expression,
            span: 0..0,
        }
        .to_string()
    )
}

/// What a token does to the depth of the tree a parser builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// Opens a group the parser descends into: `[`, `(`, `f(`.
    Open,
    /// Closes the innermost group.
    Close,
    /// Applies to the operand after it: `~`, `$`, `\`.
    Prefix,
    /// Applies to the operand before it: `*`, `.i`, a weight.
    Postfix,
    /// Joins the operands either side of it: `|`, `.o.`, `:`.
    Infix,
    /// An operand: a symbol, a string, a file read.
    Operand,
    /// Ends an expression: `;`, the `,` between arguments, a rule arrow.
    End,
    /// Changes no depth.
    Neutral,
}

/// How deep a part of a source goes: its tree, and the evaluation of it.
#[derive(Clone, Copy, Default)]
struct Depth {
    tree: usize,
    eval: usize,
}

impl Depth {
    fn max(self, other: Depth) -> Depth {
        Depth {
            tree: self.tree.max(other.tree),
            eval: self.eval.max(other.eval),
        }
    }

    fn plus(self, levels: usize) -> Depth {
        Depth {
            tree: self.tree + levels,
            eval: self.eval + levels,
        }
    }
}

/// One group of the source, or its top level.
#[derive(Default)]
struct Frame {
    /// The prefix operators the group is the operand of.
    held: usize,
    /// Operators joining the operands of the expression read so far, and
    /// juxtapositions, each a level of the tree above its operands.
    ops: usize,
    /// The deepest operand of the expression read so far.
    deepest: Depth,
    /// The operand being read, with the postfix operators read after it.
    operand: Option<Depth>,
    /// Prefix operators waiting for their operand.
    pending: usize,
    /// The deepest expression of the group ended so far.
    ended: Depth,
}

/// How an evaluator takes a chain of operators.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chains {
    /// In a loop, as XRE's does: a chain nests evaluation no deeper than
    /// the operators' precedence levels do.
    Loop,
    /// By recursing, a level for each operator, as pmatch's and twolc's do.
    Recurse,
}

/// A bound on the levels of evaluation that XRE's precedence nests a chain
/// of operators into: its grammar has fewer levels of binary operators than
/// this.
const PRECEDENCE_LEVELS: usize = 12;

/// The depth a source's tokens parse to, measured from the tokens alone.
///
/// The depths are upper bounds: each operator of an expression counts a
/// level of the tree, as a chain of one operator nests, though operators of
/// different precedence nest less.
pub(crate) struct Scan {
    frames: Vec<Frame>,
    chains: Chains,
    /// [`MAX_EXPRESSION_DEPTH`] less what this thread's compiles already hold.
    eval_limit: usize,
    /// The deepest expression ended at the top level since
    /// [`Scan::take_statement`].
    statement: Depth,
}

impl Scan {
    pub(crate) fn new(chains: Chains) -> Scan {
        Scan {
            frames: vec![Frame::default()],
            chains,
            eval_limit: MAX_EXPRESSION_DEPTH.saturating_sub(depth()),
            statement: Depth::default(),
        }
    }

    fn top(&mut self) -> &mut Frame {
        self.frames
            .last_mut()
            .expect("the top level is never closed")
    }

    fn check(&self, depth: Depth, span: &Range<usize>) -> Result<(), TooDeep> {
        let limit = if depth.eval > self.eval_limit {
            Limit::Expression
        } else if depth.tree > MAX_TREE_DEPTH {
            Limit::Tree
        } else {
            return Ok(());
        };
        Err(TooDeep {
            limit,
            span: span.clone(),
        })
    }

    /// An operand begins: one read before it, with no operator between,
    /// is juxtaposed with it.
    fn begin_operand(&mut self) {
        let frame = self.top();
        if let Some(operand) = frame.operand.take() {
            frame.deepest = frame.deepest.max(operand);
            frame.ops += 1;
        }
    }

    fn end_operand(&mut self) {
        let frame = self.top();
        if let Some(operand) = frame.operand.take() {
            frame.deepest = frame.deepest.max(operand);
        }
    }

    /// End the expression of the innermost group, returning the group's
    /// deepest expression.
    fn end_expression(&mut self) -> Depth {
        self.end_operand();
        let chains = self.chains;
        let frame = self.top();
        let chain = match chains {
            Chains::Loop => frame.ops.min(PRECEDENCE_LEVELS),
            Chains::Recurse => frame.ops,
        };
        let depth = Depth {
            tree: frame.deepest.tree + frame.ops,
            eval: frame.deepest.eval + chain,
        }
        .plus(frame.pending);
        frame.ended = frame.ended.max(depth);
        frame.ops = 0;
        frame.deepest = Depth::default();
        frame.pending = 0;
        frame.ended
    }

    pub(crate) fn token(&mut self, role: Role, span: &Range<usize>) -> Result<(), TooDeep> {
        match role {
            Role::Operand => {
                self.begin_operand();
                let frame = self.top();
                let levels = 1 + std::mem::take(&mut frame.pending);
                frame.operand = Some(Depth::default().plus(levels));
            }
            Role::Prefix => {
                self.begin_operand();
                self.top().pending += 1;
            }
            Role::Postfix => {
                let frame = self.top();
                frame.operand = Some(frame.operand.unwrap_or(Depth::default().plus(1)).plus(1));
            }
            Role::Infix => {
                self.end_operand();
                self.top().ops += 1;
            }
            Role::Open => {
                self.begin_operand();
                let held = std::mem::take(&mut self.top().pending);
                self.frames.push(Frame {
                    held,
                    ..Frame::default()
                });
                if self.frames.len() - 1 > MAX_NESTING_DEPTH {
                    return Err(TooDeep {
                        limit: Limit::Nesting,
                        span: span.clone(),
                    });
                }
            }
            Role::Close if self.frames.len() > 1 => {
                let ended = self.end_expression();
                let group = self.frames.pop().expect("a group is open");
                let depth = ended.plus(1 + group.held);
                self.check(depth, span)?;
                self.top().operand = Some(depth);
            }
            Role::Close | Role::End => {
                let ended = self.end_expression();
                self.check(ended, span)?;
                if self.frames.len() == 1 {
                    self.statement = self.statement.max(ended);
                    self.top().ended = Depth::default();
                }
            }
            Role::Neutral => {}
        }
        Ok(())
    }

    /// How deep evaluating the top-level expressions ended since the last
    /// call goes.
    pub(crate) fn take_statement(&mut self) -> usize {
        std::mem::take(&mut self.statement).eval
    }

    /// End the source at `end`, closing what is still open, and return how
    /// deep evaluating it goes since [`Scan::take_statement`].
    pub(crate) fn finish(&mut self, end: usize) -> Result<usize, TooDeep> {
        let span = end..end;
        while self.frames.len() > 1 {
            self.token(Role::Close, &span)?;
        }
        self.token(Role::End, &span)?;
        Ok(self.take_statement())
    }
}

/// The role of an XRE token.
pub(crate) fn xre_role(token: &nfst_xre::Token) -> Role {
    use nfst_xre::Token as T;
    match token {
        T::LeftBracket | T::LeftParenthesis | T::LeftBracketDotted | T::FunctionName(_) => {
            Role::Open
        }
        T::RightBracket | T::RightParenthesis | T::RightBracketDotted => Role::Close,
        T::Complement
        | T::TermComplement
        | T::Containment
        | T::ContainmentOnce
        | T::ContainmentOpt => Role::Prefix,
        T::Star
        | T::Plus
        | T::Reverse
        | T::Invert
        | T::XreUpper
        | T::XreLower
        | T::CatenateN(_)
        | T::CatenateNPlus(_)
        | T::CatenateNMinus(_)
        | T::CatenateNToK(..)
        | T::Weight(_) => Role::Postfix,
        T::Intersection
        | T::Minus
        | T::Union
        | T::Before
        | T::After
        | T::Ignoring
        | T::PairSeparator
        | T::IgnoreInternally
        | T::Shuffle
        | T::Composition
        | T::LenientComposition
        | T::MergeRightArrow
        | T::MergeLeftArrow
        | T::CrossProduct
        | T::UpperPriorityUnion
        | T::LowerPriorityUnion
        | T::UpperMinus
        | T::LowerMinus
        | T::LeftRestriction
        | T::LeftRightArrow
        | T::LeftArrow
        | T::RightArrow
        | T::ReplaceRight
        | T::OptionalReplaceRight
        | T::ReplaceLeft
        | T::OptionalReplaceLeft
        | T::ReplaceLeftRight
        | T::OptionalReplaceLeftRight
        | T::LtrLongestMatch
        | T::LtrShortestMatch
        | T::RtlLongestMatch
        | T::RtlShortestMatch
        | T::ReplaceContextUu
        | T::ReplaceContextLu
        | T::ReplaceContextUl
        | T::ReplaceContextLl
        | T::MarkupMarker
        | T::LeftQuotient => Role::Infix,
        T::AnyToken
        | T::QuotedLiteral(_)
        | T::CurlyBrackets(_)
        | T::ReadBin(_)
        | T::ReadText(_)
        | T::ReadSpaced(_)
        | T::ReadProlog(_)
        | T::ReadRe(_)
        | T::Symbol(_)
        | T::MultiCharSymbol(_)
        | T::EpsilonToken => Role::Operand,
        T::EndOfExpression | T::Comma | T::Commacomma | T::CenterMarker => Role::End,
        T::SubstituteLeft => Role::Neutral,
    }
}

// [spec:hfst:req:embed.no-panic]
/// Measure the XRE source `src`. One that does not tokenize passes: its
/// parse fails before it descends.
pub(crate) fn scan_xre(src: &str) -> Result<usize, TooDeep> {
    let Ok(tokens) = nfst_xre::tokenize(src) else {
        return Ok(0);
    };
    let mut scan = Scan::new(Chains::Loop);
    for (token, span) in &tokens {
        scan.token(xre_role(token), &span.range)?;
    }
    scan.finish(src.len())
}

/// The role of a pmatch token.
fn pmatch_role(token: &nfst_pmatch::Token) -> Role {
    use nfst_pmatch::Token as T;
    match token {
        T::LitLeft
        | T::InsLeft
        | T::EndTagLeft
        | T::CaptureLeft
        | T::CapLeft
        | T::OptCapLeft
        | T::ToLowerLeft
        | T::ToUpperLeft
        | T::OptToLowerLeft
        | T::OptToUpperLeft
        | T::AnyCaseLeft
        | T::ExplodeLeft
        | T::ImplodeLeft
        | T::LcLeft
        | T::RcLeft
        | T::NlcLeft
        | T::NrcLeft
        | T::OrLeft
        | T::AndLeft
        | T::TagLeft
        | T::WithLeft
        | T::LstLeft
        | T::ExcLeft
        | T::LikeLeft
        | T::UnlikeLeft
        | T::InterpolateLeft
        | T::SigmaLeft
        | T::CounterLeft
        | T::DefineLeft
        | T::UncomposeLeft
        | T::SymbolWithLeftParen(_)
        | T::LeftBracket
        | T::LeftParenthesis
        | T::LeftBracketDotted => Role::Open,
        T::RightBracket | T::RightParenthesis | T::RightBracketDotted => Role::Close,
        T::Complement
        | T::TermComplement
        | T::Containment
        | T::ContainmentOnce
        | T::ContainmentOpt
        | T::PairSeparatorWoLeft => Role::Prefix,
        T::Star
        | T::Plus
        | T::Reverse
        | T::Invert
        | T::UpperProject
        | T::LowerProject
        | T::CatenateN(_)
        | T::CatenateNPlus(_)
        | T::CatenateNMinus(_)
        | T::CatenateNToK(..)
        | T::Weight(_)
        | T::PairSeparatorWoRight => Role::Postfix,
        // A pair's lower side is parsed by descending, so a chain of them
        // nests as any chain does.
        T::PairSeparator
        | T::Intersection
        | T::Minus
        | T::Union
        | T::IgnoreInternally
        | T::Ignoring
        | T::Composition
        | T::LenientComposition
        | T::MergeRightArrow
        | T::MergeLeftArrow
        | T::CrossProduct
        | T::UpperPriorityUnion
        | T::LowerPriorityUnion
        | T::UpperMinus
        | T::LowerMinus
        | T::LeftRestriction
        | T::LeftRightArrow
        | T::LeftArrow
        | T::RightArrow
        | T::ReplaceRight
        | T::OptionalReplaceRight
        | T::ReplaceLeft
        | T::OptionalReplaceLeft
        | T::ReplaceLeftRight
        | T::OptionalReplaceLeftRight
        | T::LtrLongestMatch
        | T::LtrShortestMatch
        | T::RtlLongestMatch
        | T::RtlShortestMatch
        | T::ReplaceContextUu
        | T::ReplaceContextLu
        | T::ReplaceContextUl
        | T::ReplaceContextLl
        | T::MarkupMarker
        | T::LeftQuotient
        | T::Shuffle
        | T::Before
        | T::After => Role::Infix,
        T::Alpha
        | T::UppercaseAlpha
        | T::LowercaseAlpha
        | T::Num
        | T::Punct
        | T::Whitespace
        | T::VariableName(_)
        | T::EpsilonToken
        | T::AnyToken
        | T::BoundaryMarker
        | T::PairSeparatorSole
        | T::Symbol(_)
        | T::QuotedLiteral(_)
        | T::CurlyLiteral(_)
        | T::CharacterRange(..)
        | T::ReadBin(_)
        | T::ReadText(_)
        | T::ReadSpaced(_)
        | T::ReadProlog(_)
        | T::ReadLexc(_)
        | T::ReadRe(_) => Role::Operand,
        T::EndOfWeightedExpression(_)
        | T::Comma
        | T::Commacomma
        | T::CenterMarker
        | T::Define
        | T::DefIns
        | T::Regex
        | T::SetVariable
        | T::DefinedList
        | T::ReadVec(_) => Role::End,
        T::SubstituteLeft | T::Equals => Role::Neutral,
    }
}

/// Where a pmatch statement's tokens are, for what a statement names.
enum Statement {
    /// Before a statement's keyword.
    Between,
    /// The name after `Define`, `DefIns` or `list`, or `set`'s variable.
    Name { define: bool, set: bool },
    /// A function's parameters, up to the `)` that ends them.
    Parameters,
    /// `set`'s value.
    Value,
    /// The body of the named definition.
    Body,
}

// [spec:hfst:req:embed.no-panic]
/// Measure the pmatch script `src`, returning the depth of each definition
/// and function by name, `TOP` for a `regex` statement. One that does not
/// tokenize passes with no depths: its parse fails before it descends.
pub(crate) fn scan_pmatch(src: &str) -> Result<std::collections::HashMap<String, usize>, TooDeep> {
    use nfst_pmatch::Token as T;
    let mut depths = std::collections::HashMap::new();
    let Ok(tokens) = nfst_pmatch::tokenize(src) else {
        return Ok(depths);
    };
    let mut scan = Scan::new(Chains::Recurse);
    let mut statement = Statement::Between;
    let mut name = String::new();
    for (token, span) in &tokens {
        let role = match (&statement, token) {
            (Statement::Name { define: true, .. }, T::SymbolWithLeftParen(function)) => {
                name = function.trim_end_matches('(').to_string();
                statement = Statement::Parameters;
                Role::Neutral
            }
            (Statement::Name { set, .. }, T::Symbol(named) | T::VariableName(named)) => {
                name = named.to_string();
                statement = if *set {
                    Statement::Value
                } else {
                    Statement::Body
                };
                Role::Neutral
            }
            (Statement::Parameters, T::RightParenthesis) => {
                statement = Statement::Body;
                Role::Neutral
            }
            (Statement::Parameters, _) => Role::Neutral,
            (Statement::Value, _) => {
                statement = Statement::Between;
                Role::Neutral
            }
            (_, T::Define | T::DefIns | T::DefinedList | T::SetVariable) => {
                statement = Statement::Name {
                    define: matches!(token, T::Define),
                    set: matches!(token, T::SetVariable),
                };
                Role::End
            }
            (_, T::Regex) => {
                statement = Statement::Body;
                name = "TOP".to_string();
                Role::End
            }
            _ => pmatch_role(token),
        };
        scan.token(role, &span.range)?;
        if matches!(token, T::EndOfWeightedExpression(_)) && scan.frames.len() == 1 {
            let depth = scan.take_statement();
            if matches!(statement, Statement::Body) {
                depths.insert(std::mem::take(&mut name), depth);
            }
            statement = Statement::Between;
        }
    }
    scan.finish(src.len())?;
    Ok(depths)
}

/// The role of a twolc token in a regular expression.
fn twolc_role(token: &nfst_twolc::Token) -> Role {
    use nfst_twolc::Token as T;
    match token {
        T::LeftBracket | T::LeftParenthesis | T::LeftCurly | T::ReLeftBracket => Role::Open,
        T::RightBracket | T::RightParenthesis | T::RightCurly | T::ReRightBracket => Role::Close,
        T::Complement | T::TermComplement | T::Containment | T::ContainmentOnce => Role::Prefix,
        T::Star | T::Plus | T::Power => Role::Postfix,
        T::Union | T::Intersection | T::Difference | T::FreelyInsert | T::Colon => Role::Infix,
        T::Symbol(_) | T::QuestionMark => Role::Operand,
        T::SectionAlphabet
        | T::SectionDiacritics
        | T::SectionSets
        | T::SectionDefinitions
        | T::SectionRules
        | T::Where
        | T::Except
        | T::Matched
        | T::Mixed
        | T::Freely
        | T::In
        | T::And
        | T::LeftArrow
        | T::RightArrow
        | T::LeftRightArrow
        | T::LeftRestrictionArrow
        | T::ReLeftArrow
        | T::ReRightArrow
        | T::ReLeftRightArrow
        | T::ReLeftRestrictionArrow
        | T::Semicolon
        | T::Equals
        | T::CenterMarker
        | T::Comma
        | T::RuleName(_) => Role::End,
    }
}

// [spec:hfst:req:embed.no-panic]
/// Measure the regular expressions of the twolc grammar `src`: its
/// definitions and its rules. The alphabet, the diacritics and the sets are
/// lists, which the parser reads without descending. A grammar that does
/// not tokenize passes: its parse fails before it descends.
pub(crate) fn scan_twolc(src: &str) -> Result<usize, TooDeep> {
    use nfst_twolc::Token as T;
    let Ok(tokens) = nfst_twolc::tokenize(src) else {
        return Ok(0);
    };
    let mut scan = Scan::new(Chains::Recurse);
    let mut lists = false;
    let mut count = false;
    for (token, span) in &tokens {
        let role = if matches!(
            token,
            T::SectionAlphabet | T::SectionDiacritics | T::SectionSets
        ) {
            lists = true;
            Role::End
        } else if matches!(token, T::SectionDefinitions | T::SectionRules) {
            lists = false;
            Role::End
        } else if lists || (count && matches!(token, T::Symbol(_))) {
            // A list, or the count after `^`, which is part of the operator.
            Role::Neutral
        } else {
            twolc_role(token)
        };
        count = matches!(token, T::Power);
        scan.token(role, &span.range)?;
    }
    scan.finish(src.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:hfst:req:embed.no-panic/test]
    #[test]
    fn twolc_alphabets_and_sets_are_lists() {
        // Read without descending, they may be as long as they like.
        let symbols: Vec<String> = (0..=MAX_EXPRESSION_DEPTH)
            .map(|i| format!("s{i}"))
            .collect();
        let pairs: Vec<String> = symbols.iter().map(|s| format!("{s}:{s}")).collect();
        let grammar = format!(
            "Alphabet a b {} ;\nSets\nS = {} ;\nRules\n\"r\"\na:b <=> _ ;\n",
            pairs.join(" "),
            symbols.join(" ")
        );
        assert_eq!(scan_twolc(&grammar), Ok(2), "the rule's centre, a:b, alone");
    }

    // [spec:hfst:req:embed.no-panic/test]
    #[test]
    fn xre_chains_nest_evaluation_by_precedence_alone() {
        let chain = vec!["a"; MAX_EXPRESSION_DEPTH * 2].join(" | ");
        assert!(scan_xre(&chain).is_ok_and(|eval| eval <= PRECEDENCE_LEVELS + 1));
        let mixed = vec!["a b"; MAX_EXPRESSION_DEPTH].join(" | ");
        assert!(scan_xre(&mixed).is_ok_and(|eval| eval <= PRECEDENCE_LEVELS + 1));
    }

    // [spec:hfst:req:embed.no-panic/test]
    #[test]
    fn groups_hold_their_prefix_operators() {
        let src = format!("{}[a]", "~".repeat(MAX_EXPRESSION_DEPTH));
        assert_eq!(scan_xre(&src).map_err(|e| e.limit), Err(Limit::Expression));
    }

    // [spec:hfst:req:embed.no-panic/test]
    #[test]
    fn pmatch_definitions_have_their_own_depths() {
        let depths = scan_pmatch(
            "set need-separators off\nDefine A [[a]] ;\nDefine F(x) [x] ;\nregex A F(b) ;\n",
        )
        .expect("shallow");
        assert_eq!(depths.get("A"), Some(&3));
        assert_eq!(depths.get("F"), Some(&2));
        assert!(depths.contains_key("TOP"));
        assert_eq!(depths.len(), 3);
    }
}
