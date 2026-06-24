# libhfst/src/implementations/HfstTransition.h

> [spec:hfst:def:hfst-transition.hfst.hfst-basic-transition]
> typedef HfstTransition<HfstTropicalTransducerTransitionData>

> [spec:hfst:def:hfst-transition.hfst.hfst-fast-transition]
> typedef HfstTransition<HfstFastTransitionData> HfstFastTransition

> [spec:hfst:def:hfst-transition.hfst.implementations.get-input-number-fn]
> HFSTDLL unsigned int get_input_number() const

> [spec:hfst:sem:hfst-transition.hfst.implementations.get-input-number-fn]
> Const member accessor. Returns `transition_data.get_input_number()`,
> i.e. delegates to the member `transition_data` (of type C) to obtain
> the internal input symbol number as an `unsigned int`. No state is
> mutated; no side effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.get-output-number-fn]
> HFSTDLL unsigned int get_output_number() const

> [spec:hfst:sem:hfst-transition.hfst.implementations.get-output-number-fn]
> Const member accessor. Returns `transition_data.get_output_number()`,
> i.e. delegates to the member `transition_data` (of type C) to obtain
> the internal output symbol number as an `unsigned int`. No state is
> mutated; no side effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.get-target-state-fn]
> HFSTDLL HfstState get_target_state() const

> [spec:hfst:sem:hfst-transition.hfst.implementations.get-target-state-fn]
> Const member accessor. Returns the value of the member `target_state`
> (type `HfstState`), the state this transition leads to. No state is
> mutated; no side effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition]
> class HfstTransition {
>   HfstState target_state;
>   C transition_data;
> }

> [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition-fn]
> HFSTDLL ~HfstTransition()

> [spec:hfst:sem:hfst-transition.hfst.implementations.hfst-transition-fn]
> Destructor `~HfstTransition()`. Empty body; performs no explicit
> cleanup. Member subobjects (`target_state` and `transition_data`) are
> destroyed by their own destructors via the normal C++ destruction
> sequence. In Rust this corresponds to the default automatic drop; no
> custom Drop logic is needed.

> [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition.get-symbol-number-fn]
> static unsigned int get_symbol_number

> [spec:hfst:sem:hfst-transition.hfst.implementations.hfst-transition.get-symbol-number-fn]
> Protected static helper. Takes a `const C::SymbolType & symbol` and
> returns `C::get_symbol_number(symbol)`, i.e. forwards to the static
> `get_symbol_number` of the transition-data type C to map a symbol to
> its internal number (`unsigned int`). No instance state involved; no
> side effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.hfst-transition.hfst-transition-fn]
> HFSTDLL HfstTransition(HfstState s,

> [spec:hfst:sem:hfst-transition.hfst.implementations.hfst-transition.hfst-transition-fn]
> Constructor taking `(HfstState s, unsigned int inumber, unsigned int
> onumber, C::WeightType weight, bool foo)`. Initializes member
> `target_state` to `s`, and constructs member `transition_data` from
> `(inumber, onumber, weight)` using C's number-based constructor. The
> trailing `bool foo` parameter is ignored (`(void)foo;` in the body) —
> it exists only to disambiguate this number-based overload from the
> symbol-based constructor. No side effects beyond member init.

> [spec:hfst:def:hfst-transition.hfst.implementations.operator-fn]
> HFSTDLL bool operator<(const HfstTransition<C> &another) const

> [spec:hfst:sem:hfst-transition.hfst.implementations.operator-fn]
> Const less-than comparison against `another`. Compares primarily by
> `target_state`: if `target_state == another.target_state`, returns
> `transition_data < another.transition_data` (delegating to C's `<`);
> otherwise returns `target_state < another.target_state`. Provides a
> total ordering used to store transitions in a set. No mutation; no
> side effects. (In Rust: order by target_state, then by
> transition_data.)

> [spec:hfst:def:hfst-transition.hfst.implementations.set-weight-fn]
> HFSTDLL void set_weight(float w)

> [spec:hfst:sem:hfst-transition.hfst.implementations.set-weight-fn]
> Mutator. Calls `transition_data.set_weight(w)`, forwarding the `float
> w` to the member `transition_data` (type C) to update its stored
> weight. Mutates `transition_data`; returns void; no other side
> effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.typename-c.symbol-type-get-input-symbol-fn]
> HFSTDLL typename C::SymbolType get_input_symbol() const

> [spec:hfst:sem:hfst-transition.hfst.implementations.typename-c.symbol-type-get-input-symbol-fn]
> Const member accessor. Returns `transition_data.get_input_symbol()`,
> delegating to the member `transition_data` (type C) to obtain the
> input symbol as a `C::SymbolType` (returned by value). No state is
> mutated; no side effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.typename-c.symbol-type-get-output-symbol-fn]
> HFSTDLL typename C::SymbolType get_output_symbol() const

> [spec:hfst:sem:hfst-transition.hfst.implementations.typename-c.symbol-type-get-output-symbol-fn]
> Const member accessor. Returns `transition_data.get_output_symbol()`,
> delegating to the member `transition_data` (type C) to obtain the
> output symbol as a `C::SymbolType` (returned by value). No state is
> mutated; no side effects.

> [spec:hfst:def:hfst-transition.hfst.implementations.typename-c.weight-type-get-weight-fn]
> HFSTDLL typename C::WeightType get_weight() const

> [spec:hfst:sem:hfst-transition.hfst.implementations.typename-c.weight-type-get-weight-fn]
> Const member accessor. Returns `transition_data.get_weight()`,
> delegating to the member `transition_data` (type C) to obtain the
> weight as a `C::WeightType` (returned by value). No state is mutated;
> no side effects.

