//! Constructors for the 'PmatchObject' AST nodes.

use super::*;

// ---------------------------------------------------------------------------
// PmatchObject node constructors (literal ports of the C++ ctors; base fields
// follow 'PmatchObject::PmatchObject()' defaults: name="", weight=0.0,
// line_defined=0 (no lexer line counter in this port), my_timer=0, cache=NULL)
// ---------------------------------------------------------------------------

impl<B: AlgebraBackend + 'static> PmatchSymbol<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.pmatch-symbol-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.pmatch-symbol-fn]
    pub fn new(str: Symbol) -> Rc<PmatchSymbol<B>> {
        Rc::new(PmatchSymbol {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            sym: str,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchString<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.pmatch-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.pmatch-string-fn]
    pub fn new(str: Symbol, is_multichar: bool) -> Rc<PmatchString<B>> {
        Rc::new(PmatchString {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            string: str,
            multichar: is_multichar,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchNumericOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.pmatch-numeric-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.pmatch-numeric-operation-fn]
    pub fn new(op: PmatchNumericOp, root: ObjRef<B>) -> Rc<PmatchNumericOperation<B>> {
        Rc::new(PmatchNumericOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            root,
            values: Vec::new(),
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchUnaryOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.pmatch-unary-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.pmatch-unary-operation-fn]
    pub fn new(op: PmatchUnaryOp, root: ObjRef<B>) -> Rc<PmatchUnaryOperation<B>> {
        Rc::new(PmatchUnaryOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            root,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchBinaryOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.pmatch-binary-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.pmatch-binary-operation-fn]
    pub fn new(
        op: PmatchBinaryOp,
        left: ObjRef<B>,
        right: ObjRef<B>,
    ) -> Rc<PmatchBinaryOperation<B>> {
        Rc::new(PmatchBinaryOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            left,
            right,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchTernaryOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.pmatch-ternary-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.pmatch-ternary-operation-fn]
    pub fn new(
        op: PmatchTernaryOp,
        left: ObjRef<B>,
        middle: ObjRef<B>,
        right: ObjRef<B>,
    ) -> Rc<PmatchTernaryOperation<B>> {
        Rc::new(PmatchTernaryOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            left,
            middle,
            right,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchFunction<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.pmatch-function-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.pmatch-function-fn]
    pub fn new(argument_vector: Vec<Symbol>, function_root: ObjRef<B>) -> Rc<PmatchFunction<B>> {
        Rc::new(PmatchFunction {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            args: argument_vector,
            root: function_root,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchFuncall<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.pmatch-funcall-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.pmatch-funcall-fn]
    pub fn new(argument_vector: Vec<ObjRef<B>>, function: ObjRef<B>) -> Rc<PmatchFuncall<B>> {
        Rc::new(PmatchFuncall {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            args: argument_vector,
            fun: function,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchBuiltinFunction<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.pmatch-builtin-function-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.pmatch-builtin-function-fn]
    pub fn new(ty: PmatchBuiltin, argument_vector: Vec<ObjRef<B>>) -> Rc<PmatchBuiltinFunction<B>> {
        Rc::new(PmatchBuiltinFunction {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            args: argument_vector,
            ty,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchAcceptor<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.pmatch-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.pmatch-acceptor-fn]
    pub fn new(s: PmatchPredefined) -> Rc<PmatchAcceptor<B>> {
        Rc::new(PmatchAcceptor {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            set: s,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchObjectPair<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.pmatch-object-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.pmatch-object-pair-fn]
    pub fn new(l: ObjRef<B>, r: ObjRef<B>) -> Rc<PmatchObjectPair<B>> {
        Rc::new(PmatchObjectPair { left: l, right: r })
    }
}

impl<B: AlgebraBackend + 'static> PmatchMarkupContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.pmatch-markup-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.pmatch-markup-container-fn]
    pub fn new(loa: ObjRef<B>, lom: ObjRef<B>, rom: ObjRef<B>) -> Rc<PmatchMarkupContainer<B>> {
        Rc::new(PmatchMarkupContainer {
            left: lom,
            right: rom,
            left_of_arrow: loa,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchRestrictionContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.pmatch-restriction-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.pmatch-restriction-container-fn]
    pub fn new(l: ObjRef<B>, c: MappingPairVector<B>) -> Rc<PmatchRestrictionContainer<B>> {
        Rc::new(PmatchRestrictionContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            left: l,
            contexts: c,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchMappingPairsContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.pmatch-mapping-pairs-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.pmatch-mapping-pairs-container-fn]
    pub fn new(
        a: ReplaceArrow,
        left: ObjRef<B>,
        right: ObjRef<B>,
    ) -> Rc<PmatchMappingPairsContainer<B>> {
        let mut obj = PmatchMappingPairsContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            arrow: a,
            mapping_pairs: MappingPairVector::new(),
        };
        let pair: PairRef<B> = PmatchObjectPair::new(left, right);
        obj.mapping_pairs.push(pair);
        Rc::new(obj)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.push-back-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.push-back-fn]
    pub fn push_back(&mut self, one_pair: &PmatchMappingPairsContainer<B>) {
        for it in one_pair.mapping_pairs.iter() {
            let pair: PairRef<B> = PmatchObjectPair::new(it.get_left(), it.get_right());
            self.mapping_pairs.push(pair);
        }
    }
}

impl<B: AlgebraBackend + 'static> PmatchContextsContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.pmatch-contexts-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.pmatch-contexts-container-fn]
    pub fn new(
        t: ReplaceType,
        context: &PmatchContextsContainer<B>,
    ) -> Rc<PmatchContextsContainer<B>> {
        Rc::new(PmatchContextsContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            ty: t,
            context_pairs: context.context_pairs.clone(),
        })
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.push-back-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.push-back-fn]
    pub fn push_back(&mut self, one_context: &PmatchContextsContainer<B>) {
        for it in one_context.context_pairs.iter() {
            let pair: PairRef<B> = PmatchObjectPair::new(it.get_left(), it.get_right());
            self.context_pairs.push(pair);
        }
    }
}

impl<B: AlgebraBackend + 'static> PmatchReplaceRuleContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.pmatch-replace-rule-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.pmatch-replace-rule-container-fn]
    pub fn new(
        a: ReplaceArrow,
        t: ReplaceType,
        m: MappingPairVector<B>,
        c: MappingPairVector<B>,
    ) -> Rc<PmatchReplaceRuleContainer<B>> {
        Rc::new(PmatchReplaceRuleContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            arrow: a,
            ty: t,
            mapping: m,
            context: c,
        })
    }
}
