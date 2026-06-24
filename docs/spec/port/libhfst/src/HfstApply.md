# libhfst/src/HfstApply.cc

> [spec:hfst:def:hfst-apply.another-fn]
> HfstTransducer another(another_tr)

> [spec:hfst:sem:hfst-apply.another-fn]
> This is the local working copy `another` constructed inside the binary
> `HfstTransducer::apply(...)` overload that takes `HfstTransducer &another_tr`
> and `bool harmonize`. It is a copy-constructed `HfstTransducer` (`HfstTransducer
> another(another_tr);`) made from the caller's `another_tr` so that the
> subsequent alphabet insertion and harmonization steps mutate this private copy
> rather than the caller's argument.
> Context of how it is used: the apply overload first throws
> `TransducerTypeMismatchException` if `this->type != another_tr.type`. Then it
> constructs this copy. If `harmonize` is false, it calls
> `this->insert_missing_symbols_to_alphabet_from(another)` and
> `another.insert_missing_symbols_to_alphabet_from(*this)` to make the two
> alphabets agree without true harmonization. Regardless of `harmonize`, it then
> calls `this->insert_missing_symbols_to_alphabet_from(another, true)` and
> `another.insert_missing_symbols_to_alphabet_from(*this, true)` to share special
> symbols (special symbols are never harmonized). It then computes
> `HfstTransducer * another_ = this->harmonize_(another);`; if `harmonize_`
> returns NULL (the foma case), it sets `another_ = new HfstTransducer(another)`.
> The backend-specific binary function is then applied to `this`'s implementation
> and `another_`'s implementation, replacing `this`'s implementation with the
> result (deleting the old one), and finally `another_` is deleted before
> returning `*this`.

> [spec:hfst:def:hfst-apply.hfst.hfst-transducer.is-safe-conversion-fn]
> bool HfstTransducer::is_safe_conversion

> [spec:hfst:sem:hfst-apply.hfst.hfst-transducer.is-safe-conversion-fn]
> Static-style member `bool HfstTransducer::is_safe_conversion(ImplementationType
> original, ImplementationType converted)`. Returns whether converting a
> transducer from implementation type `original` to type `converted` is "safe",
> meaning no weights or other information are lost. Pure function; reads only its
> two arguments, mutates nothing, no I/O, no exceptions.
> Decision logic, in order:
> 1. If `original == converted`, return `true`.
> 2. If `original == TROPICAL_OPENFST_TYPE` and `converted == LOG_OPENFST_TYPE`,
>    return `false`.
> 3. If `original == LOG_OPENFST_TYPE` and `converted == TROPICAL_OPENFST_TYPE`,
>    return `false`.
> 4. If `original` is either `TROPICAL_OPENFST_TYPE` or `LOG_OPENFST_TYPE`: if
>    `converted` is `SFST_TYPE`, return `false`; if `converted` is `FOMA_TYPE`,
>    return `false`; if `converted` is `XFSM_TYPE`, return `false`.
> 5. Otherwise return `true`.

> [spec:hfst:def:hfst-apply.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-apply.main-fn]
> Unit-test stub compiled only when the `MAIN_TEST` macro is defined. Prints
> `"Unit tests for " __FILE__ ":"` followed by a newline to `std::cout`, then
> prints `"ok"` followed by a newline to `std::cout`, then returns `0`. Performs
> no actual testing; `argc` and `argv` are unused.

