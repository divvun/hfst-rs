# libhfst/src/HfstExceptionDefs.cc, libhfst/src/HfstExceptionDefs.h

> [spec:hfst:def:hfst-exception-defs.hfst-exception]
> struct HfstException {
>   std::string name;
>   std::string file;
>   size_t line;
>   HFSTDLL std::string operator()(void) const;
>   HFSTDLL std::string what() const;
> }

> [spec:hfst:def:hfst-exception-defs.hfst-exception.hfst-exception-fn]
> HfstException::HfstException

> [spec:hfst:sem:hfst-exception-defs.hfst-exception.hfst-exception-fn]
> Constructor taking `name`, `file` (both `const std::string&`) and `line`
> (`size_t`). Copies each argument into the corresponding member field
> (`name`, `file`, `line`) via member initializer list. No other side effects.
> (Note: there is also a separate default constructor `HfstException(void)`
> that leaves members default-initialized, and an empty destructor.)

> [spec:hfst:def:hfst-exception-defs.hfst-exception.operator-fn]
> std::string HfstException::operator() (void) const

> [spec:hfst:sem:hfst-exception-defs.hfst-exception.operator-fn]
> Const member taking no arguments, returning `std::string`. Builds and
> returns the message string by concatenating, in order: the literal
> `"Exception: "`, the member `name`, the literal `" in file: "`, the member
> `file`, the literal `" on line: "`, and the member `line` (formatted as a
> decimal integer). No state mutated; no side effects beyond constructing the
> returned string.

> [spec:hfst:def:hfst-exception-defs.hfst-exception.what-fn]
> std::string HfstException::what() const

> [spec:hfst:sem:hfst-exception-defs.hfst-exception.what-fn]
> Const member taking no arguments, returning `std::string`. Identical in
> behaviour to `operator()`: builds and returns the message string by
> concatenating `"Exception: "`, the member `name`, `" in file: "`, the member
> `file`, `" on line: "`, and the member `line` (formatted as a decimal
> integer). No state mutated; no side effects beyond constructing the returned
> string.

> [spec:hfst:def:hfst-exception-defs.hfst-get-exception-fn]
> std::string hfst_get_exception()

> [spec:hfst:sem:hfst-exception-defs.hfst-get-exception-fn]
> Free function taking no arguments, returning `std::string`. Returns a copy
> of the global variable `hfst_exception` (a file-scope `std::string`, default
> empty). No mutation, no side effects.

> [spec:hfst:def:hfst-exception-defs.hfst-set-exception-fn]
> void hfst_set_exception(std::string name)

> [spec:hfst:sem:hfst-exception-defs.hfst-set-exception-fn]
> Free function taking `name` (`std::string`, by value), returning `void`.
> Assigns `name` into the global variable `hfst_exception` (a file-scope
> `std::string`), overwriting its previous value. No return value, no other
> side effects.

> [spec:hfst:def:hfst-exception-defs.implementation-type-not-available-exception]
> class ImplementationTypeNotAvailableException : public HfstException {
>   hfst::ImplementationType type;
> }

> [spec:hfst:def:hfst-exception-defs.implementation-type-not-available-exception.get-type-fn]
> hfst::ImplementationType ImplementationTypeNotAvailableException::get_type() const

> [spec:hfst:sem:hfst-exception-defs.implementation-type-not-available-exception.get-type-fn]
> Const member taking no arguments, returning `hfst::ImplementationType`.
> Returns a copy of the member field `type`. No mutation, no side effects.

> [spec:hfst:def:hfst-exception-defs.implementation-type-not-available-exception.implementation-type-not-available-exception-fn]
> ImplementationTypeNotAvailableException::ImplementationTypeNotAvailableException(const std::string &name,const std::string &file,size_t line, hfst::Implement...

> [spec:hfst:sem:hfst-exception-defs.implementation-type-not-available-exception.implementation-type-not-available-exception-fn]
> Constructor taking `name`, `file` (both `const std::string&`), `line`
> (`size_t`), and `type` (`hfst::ImplementationType`). Forwards `name`, `file`,
> and `line` to the base-class `HfstException` constructor (which copies them
> into the base members), then copies `type` into the derived member `type`.
> No other side effects.

