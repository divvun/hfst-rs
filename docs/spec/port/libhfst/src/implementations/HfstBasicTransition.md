# libhfst/src/implementations/HfstBasicTransition.cc, libhfst/src/implementations/HfstBasicTransition.h

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition]
> class HfstBasicTransition {
>   HfstState target_state;
>   HfstTropicalTransducerTransitionData transition_data;
>   HFSTDLL const HfstTropicalTransducerTransitionData & get_transition_data() const;
>   HFSTDLL HfstTropicalTransducerTransitionData::SymbolType get_input_symbol() const;
>   HFSTDLL HfstTropicalTransducerTransitionData::SymbolType get_output_symbol() const;
>   HFSTDLL HfstTropicalTransducerTransitionData::WeightType get_weight() const;
> }

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-number-fn]
> unsigned int HfstBasicTransition::get_input_number() const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-number-fn]
> Const getter. Returns `transition_data.get_input_number()` — the
> numeric input-symbol identifier of the embedded transition data.
> No mutation, no side effects.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-symbol-fn]
> HfstTropicalTransducerTransitionData::SymbolType HfstBasicTransition::get_input_symbol() const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-input-symbol-fn]
> Const getter. Returns `transition_data.get_input_symbol()` — the
> input symbol (SymbolType, the string symbol) of the embedded
> transition data. No mutation, no side effects.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-number-fn]
> unsigned int HfstBasicTransition::get_output_number() const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-number-fn]
> Const getter. Returns `transition_data.get_output_number()` — the
> numeric output-symbol identifier of the embedded transition data.
> No mutation, no side effects.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-symbol-fn]
> HfstTropicalTransducerTransitionData::SymbolType HfstBasicTransition::get_output_symbol() const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-output-symbol-fn]
> Const getter. Returns `transition_data.get_output_symbol()` — the
> output symbol (SymbolType, the string symbol) of the embedded
> transition data. No mutation, no side effects.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-target-state-fn]
> HfstState HfstBasicTransition::get_target_state() const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-target-state-fn]
> Const getter. Returns the `target_state` member (an HfstState) by
> value. No mutation, no side effects.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-weight-fn]
> HfstTropicalTransducerTransitionData::WeightType HfstBasicTransition::get_weight() const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.get-weight-fn]
> Const getter. Returns `transition_data.get_weight()` — the weight
> (WeightType, a float) of the embedded transition data. No mutation,
> no side effects.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.hfst-basic-transition-fn]
> HfstBasicTransition::HfstBasicTransition(HfstState s,

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.hfst-basic-transition-fn]
> Constructor taking a target state `s` (HfstState), an input number
> `inumber` (unsigned int), an output number `onumber` (unsigned int),
> a `weight` (WeightType), and a `bool foo`. Initializes member
> `target_state` from `s` and constructs member `transition_data` from
> `(inumber, onumber, weight)` — i.e. the number-based
> HfstTropicalTransducerTransitionData constructor. The `foo` parameter
> is unused (cast to void); it exists only to disambiguate this
> number-typed overload from the symbol-typed constructor. The body is
> empty.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.operator-fn]
> bool HfstBasicTransition::operator<(const HfstBasicTransition &another) const

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.operator-fn]
> Const less-than comparison against `another`. If this
> `target_state` equals `another.target_state`, returns
> `transition_data < another.transition_data` (delegating to the
> transition-data ordering). Otherwise returns
> `target_state < another.target_state`. Thus orders primarily by
> target state, then by transition data. No mutation.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-input-symbol-fn]
> void HfstBasicTransition::set_input_symbol(const HfstTropicalTransducerTransitionData::SymbolType & symbol)

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-input-symbol-fn]
> Mutator. Calls `transition_data.set_input_symbol(symbol)`, setting
> the embedded transition data's input symbol to the given SymbolType
> `symbol`. Returns void.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-output-symbol-fn]
> void HfstBasicTransition::set_output_symbol(const HfstTropicalTransducerTransitionData::SymbolType & symbol)

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-output-symbol-fn]
> Mutator. Calls `transition_data.set_output_symbol(symbol)`, setting
> the embedded transition data's output symbol to the given SymbolType
> `symbol`. Returns void.

> [spec:hfst:def:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-weight-fn]
> void HfstBasicTransition::set_weight(HfstTropicalTransducerTransitionData::WeightType w)

> [spec:hfst:sem:hfst-basic-transition.hfst.implementations.hfst-basic-transition.set-weight-fn]
> Mutator. Calls `transition_data.set_weight(w)`, setting the embedded
> transition data's weight to the given WeightType `w`. Returns void.

