//! `hfst reweight`: changing the weights of the arcs and final states that a
//! rule selects, for one rule or for each line of a TSV file.

use crate::backend::AlgebraBackend;
use crate::hfst_transducer::HfstTransducer;
use crate::tools::format::parse_weight;
use crate::tools::metadata::{hfst_set_formula_unary, hfst_set_name_unary};

// [spec:hfst:def:hfst-reweight.func-fn]
// [spec:hfst:sem:hfst-reweight.func-fn]
// [spec:hfst:req:embed.tools-in-library]
/// The function `name` names among those `hfst reweight --function` takes.
pub fn weight_function(name: &str) -> Option<fn(f32) -> f32> {
    let function: fn(f32) -> f32 = match name {
        "cos" => f32::cos,
        "sin" => f32::sin,
        "tan" => f32::tan,
        "acos" => f32::acos,
        "asin" => f32::asin,
        "atan" => f32::atan,
        "cosh" => f32::cosh,
        "sinh" => f32::sinh,
        "tanh" => f32::tanh,
        "exp" => f32::exp,
        "log" => f32::ln,
        "log10" => f32::log10,
        "sqrt" => f32::sqrt,
        "floor" => f32::floor,
        "ceil" => f32::ceil,
        _ => return None,
    };
    Some(function)
}

// [spec:hfst:def:hfst-reweight.id-fn]
// [spec:hfst:sem:hfst-reweight.id-fn]
fn id(w: f32) -> f32 {
    w
}

/// A reweighting rule: weights within the bounds, on the arcs and final
/// states it selects, become `multiplier * function(w) + addition`.
#[derive(Clone, Debug)]
pub struct Reweighting {
    pub addition: f32,
    pub multiplier: f32,
    pub function: fn(f32) -> f32,
    pub lower_bound: f32,
    pub upper_bound: f32,
    /// Only arcs with this symbol on either side.
    pub symbol: Option<String>,
    /// Only arcs with this input symbol, or this output symbol when
    /// `output_symbol` is given too.
    pub input_symbol: Option<String>,
    pub output_symbol: Option<String>,
    /// Final weights only.
    pub ends_only: bool,
    /// Arc weights only.
    pub arcs_only: bool,
}

impl Default for Reweighting {
    fn default() -> Self {
        Self {
            addition: 0.0,
            multiplier: 1.0,
            function: id,
            lower_bound: 0.0,
            upper_bound: f32::MAX,
            symbol: None,
            input_symbol: None,
            output_symbol: None,
            ends_only: false,
            arcs_only: false,
        }
    }
}

impl Reweighting {
    // [spec:hfst:def:hfst-reweight.reweight-fn]
    // [spec:hfst:sem:hfst-reweight.reweight-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// The new weight for weight `w` on an arc `i:o`, or on a final state
    /// when both are `None`.
    pub fn reweight(&self, w: f32, i: Option<&str>, o: Option<&str>) -> f32 {
        if w < self.lower_bound || w > self.upper_bound {
            return w;
        }
        let applies = match (i, o) {
            (Some(i), Some(o)) => !self.ends_only && self.selects_arc(i, o),
            (None, None) => !self.arcs_only,
            (Some(_), None) | (None, Some(_)) => true,
        };
        if !applies {
            return w;
        }
        self.multiplier * (self.function)(w) + self.addition
    }

    fn selects_arc(&self, i: &str, o: &str) -> bool {
        if let Some(symbol) = &self.symbol
            && i != symbol
            && o != symbol
        {
            return false;
        }
        match (&self.input_symbol, &self.output_symbol) {
            (Some(input), Some(output)) => i == input || o == output,
            (Some(input), None) => i == input,
            (None, Some(output)) => o == output,
            (None, None) => true,
        }
    }

    // [spec:hfst:def:hfst-reweight.original-fn]
    // [spec:hfst:sem:hfst-reweight.original-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Reweight `t` by this rule.
    pub fn apply<B: AlgebraBackend>(&self, t: &mut HfstTransducer<B>) -> crate::error::Result<()> {
        let original = t.to_basic()?;
        let replication = original.transform_weights(|w, i, o| self.reweight(w, i, o));
        *t = HfstTransducer::new_from_basic(&replication)?;
        Ok(())
    }
}

/// Why a reweighting TSV line was refused. Each `Display`s as the message
/// the tool prints for it.
#[derive(Clone, Debug)]
pub enum TsvError {
    /// The line has no tab between symbol and weight.
    NoTab,
    /// The weight is not a number; carries the text read.
    NotAWeight(String),
}

impl std::fmt::Display for TsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TsvError::NoTab => write!(f, "at least one tab required per line"),
            TsvError::NotAWeight(text) => write!(f, "{} not a weight", text),
        }
    }
}

impl Reweighting {
    // [spec:hfst:def:hfst-reweight.process-stream-fn]
    // [spec:hfst:sem:hfst-reweight.process-stream-fn]
    // [spec:hfst:req:embed.tools-in-library]
    /// Take the TSV line `line`, `SYMBOL<TAB>WEIGHT`, as the next rule: this
    /// one, for SYMBOL, with WEIGHT its multiplier, or its addition when it
    /// starts with `+`. The other of the two carries over from the lines
    /// before, as in the C++ tool. `None` for an empty or `#` comment line.
    pub fn next_tsv_rule(&mut self, line: &str) -> Result<Option<&Self>, TsvError> {
        if line.starts_with('\n') || line.starts_with('#') {
            return Ok(None);
        }
        let (symbol, rest) = line.split_once('\t').ok_or(TsvError::NoTab)?;
        let spec = rest.split('\n').next().unwrap_or_default();
        let read = |text: &str| {
            parse_weight(text).map_err(|refused| TsvError::NotAWeight(refused.to_string()))
        };
        self.symbol = Some(symbol.to_string());
        match spec.strip_prefix('+') {
            Some(addition) => self.addition = read(addition)?,
            None => self.multiplier = read(spec)?,
        }
        Ok(Some(self))
    }
}

// [spec:hfst:req:embed.tools-in-library]
/// Name `t` as `hfst reweight` names what it reweighted.
pub fn name_reweighted<B: AlgebraBackend>(t: &mut HfstTransducer<B>) {
    let unnamed = t.clone();
    hfst_set_name_unary(t, &unnamed, "reweight");
    hfst_set_formula_unary(t, &unnamed, "W");
}
