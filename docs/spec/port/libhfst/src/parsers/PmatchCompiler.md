# libhfst/src/parsers/PmatchCompiler.cc, libhfst/src/parsers/PmatchCompiler.h

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler]
> class PmatchCompiler {
>   bool flatten;
>   bool verbose;
>   bool include_cosine_distances;
>   std::string includedir;
>   std::map<std::string,hfst::HfstTransducer*> definitions_;
>   hfst::ImplementationType format_;
> }

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.compile-fn]
> std::map<std::string, HfstTransducer*>

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.compile-fn]
> Compiles a pmatch (Xerox-compatible regular expression) source string into a
> set of named transducers. Delegates entirely to the free function
> `hfst::pmatch::compile`, passing it: the input `pmatch` string; this object's
> current `definitions_` map (name -> HfstTransducer*); the configured target
> `format_` (ImplementationType); and the boolean/string fields `verbose`,
> `flatten`, `include_cosine_distances`, and `includedir`. Returns the
> `std::map<std::string, HfstTransducer*>` that the free function produces.
> Performs no other work and mutates no member state directly here (any state
> changes are whatever the free function performs on the passed `definitions_`).

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.define-fn]
> void

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.define-fn]
> Registers a named definition macro from a pmatch source string. Takes `name`
> and `pmatch` (both const string references). First calls `compile(pmatch)`
> (the member compile function above), which has the side effect of populating
> the global/module-level `definitions` map (a separate map from this object's
> `definitions_` member; `definitions` is provided by the pmatch parsing
> machinery). Then checks whether that global `definitions` map contains an
> entry for `name` (`definitions.count(name) != 0`); if so, it calls
> `->evaluate()` on the corresponding entry and stores the resulting
> HfstTransducer pointer into this object's member map `definitions_[name]`. If
> `name` is not present in the global `definitions` after compilation, nothing
> is stored. Returns void.

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.pmatch-compiler-fn]
> PmatchCompiler::PmatchCompiler()

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.pmatch-compiler-fn]
> Default constructor. Initializes the member fields via the initializer list:
> `flatten = false`, `verbose = false`, `definitions_` to an empty map, and
> `format_ = hfst::TROPICAL_OPENFST_TYPE`. The body is empty. (Note: the fields
> `include_cosine_distances` and `includedir` are not listed in the initializer
> list and are left default-constructed — `include_cosine_distances` is an
> uninitialized bool, `includedir` an empty string.) A second overloaded
> constructor `PmatchCompiler(hfst::ImplementationType impl)` is identical
> except it sets `format_ = impl` instead of the tropical-openfst default.

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-flatten-fn]
> void set_flatten(bool val)

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-flatten-fn]
> Inline setter. Assigns the parameter `val` to the member field `flatten`.
> Returns void.

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-cosine-distances-fn]
> void set_include_cosine_distances(bool val)

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-cosine-distances-fn]
> Inline setter. Assigns the parameter `val` to the member field
> `include_cosine_distances`. Returns void.

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
> void PmatchCompiler::set_include_path(std::string path)

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
> Setter. Assigns the parameter `path` (passed by value, a std::string) to the
> member field `includedir`. Returns void.

> [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-verbose-fn]
> void set_verbose(bool val)

> [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-verbose-fn]
> Inline setter. Assigns the parameter `val` to the member field `verbose`.
> Returns void.

> [spec:hfst:def:pmatch-compiler.main-fn]
> int

> [spec:hfst:sem:pmatch-compiler.main-fn]
> Unit-test entry point, compiled only under the `UNIT_TEST` build (the normal
> library build excludes it). Ignores both argc and argv. Writes the literal
> banner `"Unit tests for " __FILE__ ":"` to std::cout. The entire body of
> actual test logic (constructing PmatchCompilers in various backend formats,
> compiling sample expressions, comparing against hand-built basic transducers,
> and exercising `define`) is commented out, so nothing else executes. Finally
> writes `"ok."` followed by a newline to std::cout and returns
> `EXIT_SUCCESS`.

