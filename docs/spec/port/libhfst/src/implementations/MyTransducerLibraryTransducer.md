# libhfst/src/implementations/MyTransducerLibraryTransducer.cc, libhfst/src/implementations/MyTransducerLibraryTransducer.h

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream]
> class MyTransducerLibraryInputStream

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.close-fn]
> void MyTransducerLibraryInputStream::close(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.close-fn]
> Unimplemented stub. Takes no parameters, returns void. The body
> consists solely of `HFST_THROW(FunctionNotImplementedException)`, so
> calling it unconditionally throws a `FunctionNotImplementedException`
> and never returns normally. No state is read or mutated. Intended to
> close the input stream (a no-op if it points to standard in), but that
> behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.ignore-fn]
> void MyTransducerLibraryInputStream::ignore(unsigned int n)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.ignore-fn]
> Unimplemented stub. Takes parameter `n` (unsigned int), returns void.
> The parameter is discarded via `(void)n` and never used. The body then
> unconditionally executes `HFST_THROW(FunctionNotImplementedException)`,
> throwing a `FunctionNotImplementedException` and never returning
> normally. No state is read or mutated. Intended to extract and discard
> `n` characters from the stream, but that behavior is not implemented in
> this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-bad-fn]
> bool MyTransducerLibraryInputStream::is_bad(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-bad-fn]
> Unimplemented stub. Takes no parameters, declared to return bool but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated. Intended to report whether the stream is in a bad/error
> state, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-eof-fn]
> bool MyTransducerLibraryInputStream::is_eof(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-eof-fn]
> Unimplemented stub. Takes no parameters, declared to return bool but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated. Intended to report whether the stream is at end of
> file, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-fst-fn]
> bool MyTransducerLibraryInputStream::is_fst(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-fst-fn]
> Unimplemented stub. Takes no parameters, declared to return bool but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated. Intended to report whether the stream's next content is
> a transducer in this library's format, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-good-fn]
> bool MyTransducerLibraryInputStream::is_good(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.is-good-fn]
> Unimplemented stub. Takes no parameters, declared to return bool but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated. Intended to report whether the stream is in a good
> (non-error, non-eof) state, but that behavior is not implemented in this
> template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.my-transducer-library-input-stream-fn]
> MyTransducerLibraryInputStream::MyTransducerLibraryInputStream

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.my-transducer-library-input-stream-fn]
> Unimplemented stub constructor. Takes parameter `filename` (const
> std::string&). The parameter is discarded via `(void)filename` and never
> used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException`; the object is never successfully
> constructed. Intended to create and open an input stream reading from the
> named file, but that behavior is not implemented in this template. (Note:
> the no-argument constructor that opens standard in is a separate stub
> that likewise throws.)

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.read-transducer-fn]
> MyFst * MyTransducerLibraryInputStream::read_transducer()

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.read-transducer-fn]
> Unimplemented stub. Takes no parameters, declared to return `MyFst*` but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated and no allocation occurs. Intended to read one transducer
> from the stream and return a newly allocated `MyFst`, but that behavior
> is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.set-implementation-specific-header-data-fn]
> bool MyTransducerLibraryInputStream::set_implementation_specific_header_data

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.set-implementation-specific-header-data-fn]
> Unimplemented stub. Takes parameters `data` (StringPairVector&) and
> `index` (unsigned int), declared to return bool. Both parameters are
> discarded via `(void)data` and `(void)index` and never used. The body
> then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated. Intended (optionally) to consume implementation-
> specific key/value pairs from the file header starting at `index`, but
> that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.stream-get-fn]
> char MyTransducerLibraryInputStream::stream_get()

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.stream-get-fn]
> Unimplemented stub. Takes no parameters, declared to return char but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated. Intended to extract and return the next character from
> the stream, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.stream-unget-fn]
> void MyTransducerLibraryInputStream::stream_unget(char c)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-input-stream.stream-unget-fn]
> Unimplemented stub. Takes parameter `c` (char), returns void. The
> parameter is discarded via `(void)c` and never used. The body then
> unconditionally executes `HFST_THROW(FunctionNotImplementedException)`,
> throwing a `FunctionNotImplementedException` and never returning
> normally. No state is read or mutated. Intended to push character `c`
> back onto the stream, but that behavior is not implemented in this
> template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream]
> class MyTransducerLibraryOutputStream

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.append-implementation-specific-header-data-fn]
> void MyTransducerLibraryOutputStream

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.append-implementation-specific-header-data-fn]
> Unimplemented stub. Takes parameters `header` (std::vector<char>&) and
> `t` (MyFst*), returns void. Both parameters are discarded via
> `(void)header` and `(void)t` and never used. The body then
> unconditionally executes `HFST_THROW(FunctionNotImplementedException)`,
> throwing a `FunctionNotImplementedException` and never returning
> normally. No state is read or mutated. Intended (optionally) to append
> implementation-specific bytes describing transducer `t` to the `header`
> byte vector, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.close-fn]
> void MyTransducerLibraryOutputStream::close(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.close-fn]
> Unimplemented stub. Takes no parameters, returns void. The body consists
> solely of `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException` and never
> returns normally. No state is read or mutated. Intended to close the
> output stream (a no-op if it points to standard out), but that behavior
> is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.my-transducer-library-output-stream-fn]
> MyTransducerLibraryOutputStream::MyTransducerLibraryOutputStream

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.my-transducer-library-output-stream-fn]
> Unimplemented stub constructor. Takes parameter `filename` (const
> std::string&). The parameter is discarded via `(void)filename` and never
> used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException`; the object is never successfully
> constructed. Intended to create and open an output stream writing to the
> named file, but that behavior is not implemented in this template. (The
> no-argument constructor that opens standard out is a separate stub that
> likewise throws.)

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.write-fn]
> void MyTransducerLibraryOutputStream::write(const char &c)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.write-fn]
> Unimplemented stub. Takes parameter `c` (const char&), returns void. The
> parameter is discarded via `(void)c` and never used. The body then
> unconditionally executes `HFST_THROW(FunctionNotImplementedException)`,
> throwing a `FunctionNotImplementedException` and never returning
> normally. No state is read or mutated. Intended to write character `c` to
> the output stream, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.write-transducer-fn]
> void MyTransducerLibraryOutputStream::write_transducer(MyFst * transducer)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-output-stream.write-transducer-fn]
> Unimplemented stub. Takes parameter `transducer` (MyFst*), returns void.
> The parameter is discarded via `(void)transducer` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning normally. No state
> is read or mutated. Intended to serialize and write `transducer` to the
> output stream, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer]
> class MyTransducerLibraryTransducer

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.are-equivalent-fn]
> bool MyTransducerLibraryTransducer::are_equivalent(MyFst * t1, MyFst * t2)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.are-equivalent-fn]
> Unimplemented stub. Takes parameters `t1` and `t2` (both MyFst*),
> declared to return bool. Both parameters are discarded via `(void)t1`
> and `(void)t2` and never used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated. Intended to report whether `t1` and `t2` accept the
> same language, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.compose-fn]
> MyFst * MyTransducerLibraryTransducer::compose(MyFst * t1, MyFst * t2)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.compose-fn]
> Unimplemented stub. Takes parameters `t1` and `t2` (both MyFst*),
> declared to return `MyFst*`. Both parameters are discarded via
> `(void)t1` and `(void)t2` and never used. The body then unconditionally
> executes `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer for the composition of `t1` and `t2` (accepting
> string1:string3 when t1 accepts string1:string2 and t2 accepts
> string2:string3), but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.concatenate-fn]
> MyFst * MyTransducerLibraryTransducer::concatenate(MyFst * t1, MyFst * t2)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.concatenate-fn]
> Unimplemented stub. Takes parameters `t1` and `t2` (both MyFst*),
> declared to return `MyFst*`. Both parameters are discarded via
> `(void)t1` and `(void)t2` and never used. The body then unconditionally
> executes `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting the concatenation of any string pair of `t1`
> followed by any string pair of `t2`, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.copy-fn]
> MyFst * MyTransducerLibraryTransducer::copy(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.copy-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a newly
> allocated deep copy of transducer `t`, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.create-empty-transducer-fn]
> MyFst * MyTransducerLibraryTransducer::create_empty_transducer(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.create-empty-transducer-fn]
> Unimplemented stub. Takes no parameters, declared to return `MyFst*` but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated and no allocation occurs. Intended to return a newly
> allocated transducer that recognizes no string (empty language), but
> that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.create-epsilon-transducer-fn]
> MyFst * MyTransducerLibraryTransducer::create_epsilon_transducer(void)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.create-epsilon-transducer-fn]
> Unimplemented stub. Takes no parameters, declared to return `MyFst*` but
> never actually returns a value: the body consists solely of
> `HFST_THROW(FunctionNotImplementedException)`, so calling it
> unconditionally throws a `FunctionNotImplementedException`. No state is
> read or mutated and no allocation occurs. Intended to return a newly
> allocated transducer that recognizes exactly the empty string, but that
> behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.define-transducer-fn]
> MyFst * MyTransducerLibraryTransducer::define_transducer

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.define-transducer-fn]
> Unimplemented stub. This is the two-argument overload taking `isymbol`
> and `osymbol` (both const std::string&), declared to return `MyFst*`.
> Both parameters are discarded via `(void)isymbol` and `(void)osymbol`
> and never used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a newly
> allocated transducer recognizing the single symbol pair isymbol:osymbol,
> but that behavior is not implemented in this template. (The other
> `define_transducer` overloads — single symbol, StringPairVector,
> StringPairSet with cyclic flag, and vector<StringPairSet> — are separate
> stubs that likewise throw.)

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.determinize-fn]
> MyFst * MyTransducerLibraryTransducer::determinize(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.determinize-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a newly
> allocated deterministic transducer equivalent to `t`, but that behavior
> is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.disjunct-fn]
> MyFst * MyTransducerLibraryTransducer::disjunct

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.disjunct-fn]
> Unimplemented stub. This is the overload taking `t` (MyFst*) and `spv`
> (const StringPairVector&), declared to return `MyFst*`. Both parameters
> are discarded via `(void)t` and `(void)spv` and never used. The body
> then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer that is the union of `t` with the single path described by the
> symbol-pair vector `spv`, but that behavior is not implemented in this
> template. (The two-transducer `disjunct(t1, t2)` overload is a separate
> stub that likewise throws.)

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.extract-input-language-fn]
> MyFst * MyTransducerLibraryTransducer::extract_input_language(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.extract-input-language-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting string1:string1 whenever `t` accepts
> string1:string2 (projection onto the input side), but that behavior is
> not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.extract-output-language-fn]
> MyFst * MyTransducerLibraryTransducer::extract_output_language(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.extract-output-language-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting string2:string2 whenever `t` accepts
> string1:string2 (projection onto the output side), but that behavior is
> not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.extract-paths-fn]
> void MyTransducerLibraryTransducer::extract_paths

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.extract-paths-fn]
> Unimplemented stub. This is the callback overload taking `t` (MyFst*),
> `callback` (hfst::ExtractStringsCb&), `cycles` (int), `fd`
> (FdTable<unsigned int>*), and `filter_fd` (bool), returns void. All five
> parameters are discarded via `(void)` casts and never used. The body
> then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning normally. No state
> is read or mutated. Intended to enumerate the string paths of `t`
> (bounded by `cycles`, with optional flag-diacritic table `fd` and
> filtering controlled by `filter_fd`), invoking `callback` for each, but
> that behavior is not implemented in this template. (The
> `std::vector<MyFst*> extract_paths(MyFst*)` overload is a separate stub
> that likewise throws.)

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.get-alphabet-fn]
> StringSet MyTransducerLibraryTransducer::get_alphabet(MyFst *t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.get-alphabet-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return a
> `StringSet`. The parameter is discarded via `(void)t` and never used.
> The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated. Intended to return the set of all symbols occurring
> in transitions of `t`, but that behavior is not implemented in this
> template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.get-flag-diacritics-fn]
> FdTable<unsigned int>* MyTransducerLibraryTransducer::get_flag_diacritics

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.get-flag-diacritics-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `FdTable<unsigned int>*`. The parameter is discarded via `(void)t` and
> never used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a newly
> allocated table mapping the transducer's symbol numbers that represent
> flag diacritics in `t`, but that behavior is not implemented in this
> template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.get-symbol-pairs-fn]
> StringPairSet MyTransducerLibraryTransducer::get_symbol_pairs(MyFst *t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.get-symbol-pairs-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return a
> `StringPairSet`. The parameter is discarded via `(void)t` and never
> used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated. Intended to return the set of all input:output symbol
> pairs occurring in transitions of `t`, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.harmonize-fn]
> std::pair<MyFst*, MyFst*> MyTransducerLibraryTransducer::harmonize

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.harmonize-fn]
> Unimplemented stub. Takes parameters `t1` and `t2` (both MyFst*) and
> `unknown_symbols_in_use` (bool), declared to return
> `std::pair<MyFst*, MyFst*>`. All three parameters are discarded via
> `(void)t1`, `(void)t2`, and `(void)unknown_symbols_in_use` and never
> used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to harmonize the
> alphabets of `t1` and `t2` (reconciling unknown/identity symbols when
> `unknown_symbols_in_use` is true) and return the resulting pair of
> transducers, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.initialize-alphabet-fn]
> void MyTransducerLibraryTransducer::initialize_alphabet(MyFst *t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.initialize-alphabet-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), returns void. The
> parameter is discarded via `(void)t` and never used. The body then
> unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning normally. No state
> is read or mutated. Intended to add the standard number-to-symbol
> correspondences to the alphabet of `t` (0 : internal_epsilon
> "@_EPSILON_SYMBOL_@", 1 : internal_unknown "@_UNKNOWN_SYMBOL_@",
> 2 : internal_identity "@_IDENTITY_SYMBOL_@"), but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.insert-freely-fn]
> MyFst * MyTransducerLibraryTransducer::insert_freely

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.insert-freely-fn]
> Unimplemented stub. Takes parameters `t` (MyFst*) and `symbol_pair`
> (const StringPair&), declared to return `MyFst*`. Both parameters are
> discarded via `(void)t` and `(void)symbol_pair` and never used. The body
> then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer that accepts `symbol_pair` (call it A:B) inserted freely
> (zero or more times) between every symbol of every path of `t` — i.e. the
> pattern [ A:B* s A:B* t A:B* r A:B* i A:B* n A:B* g A:B* 1:2 A:B* ] for
> each accepted string1:string2 — but that behavior is not implemented in
> this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.intersect-fn]
> MyFst * MyTransducerLibraryTransducer::intersect(MyFst * t1, MyFst * t2)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.intersect-fn]
> Unimplemented stub. Takes parameters `t1` and `t2` (both MyFst*),
> declared to return `MyFst*`. Both parameters are discarded via
> `(void)t1` and `(void)t2` and never used. The body then unconditionally
> executes `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer that accepts exactly the string pairs accepted by both `t1`
> and `t2`, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.invert-fn]
> MyFst * MyTransducerLibraryTransducer::invert(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.invert-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer that accepts string2:string1 whenever `t` accepts
> string1:string2 (swapping input and output sides), but that behavior is
> not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.is-cyclic-fn]
> bool MyTransducerLibraryTransducer::is_cyclic(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.is-cyclic-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> bool. The parameter is discarded via `(void)t` and never used. The body
> then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated. Intended to report whether transducer `t` contains a
> cycle, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.minimize-fn]
> MyFst * MyTransducerLibraryTransducer::minimize(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.minimize-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a newly
> allocated minimal transducer equivalent to `t`, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.number-of-states-fn]
> unsigned int MyTransducerLibraryTransducer::number_of_states(MyFst *t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.number-of-states-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> unsigned int. The parameter is discarded via `(void)t` and never used.
> The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated. Intended to return the number of states in `t`, but
> that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.optionalize-fn]
> MyFst * MyTransducerLibraryTransducer::optionalize(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.optionalize-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer that accepts the string pairs accepted by `t` or the empty
> string, but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.remove-epsilons-fn]
> MyFst * MyTransducerLibraryTransducer::remove_epsilons(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.remove-epsilons-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> epsilon-free transducer equivalent to `t`, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.remove-from-alphabet-fn]
> MyFst * MyTransducerLibraryTransducer::remove_from_alphabet

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.remove-from-alphabet-fn]
> Unimplemented stub. Takes parameters `t` (MyFst*) and `symbol` (const
> std::string&), declared to return `MyFst*`. Both parameters are
> discarded via `(void)t` and `(void)symbol` and never used. The body then
> unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a
> transducer like `t` but with `symbol` removed from its alphabet, but that
> behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-le-n-fn]
> MyFst * MyTransducerLibraryTransducer::repeat_le_n(MyFst * t,int n)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-le-n-fn]
> Unimplemented stub. Takes parameters `t` (MyFst*) and `n` (int),
> declared to return `MyFst*`. Both parameters are discarded via `(void)t`
> and `(void)n` and never used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting from zero up to `n` consecutive string pairs of `t`,
> but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-n-fn]
> MyFst * MyTransducerLibraryTransducer::repeat_n(MyFst * t,int n)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-n-fn]
> Unimplemented stub. Takes parameters `t` (MyFst*) and `n` (int),
> declared to return `MyFst*`. Both parameters are discarded via `(void)t`
> and `(void)n` and never used. The body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting exactly `n` consecutive string pairs of `t`, but
> that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-plus-fn]
> MyFst * MyTransducerLibraryTransducer::repeat_plus(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-plus-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting one or more consecutive string pairs of `t` (Kleene
> plus), but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-star-fn]
> MyFst * MyTransducerLibraryTransducer::repeat_star(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.repeat-star-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting any number (zero or more) of consecutive string
> pairs of `t` (Kleene star), but that behavior is not implemented in this
> template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.reverse-fn]
> MyFst * MyTransducerLibraryTransducer::reverse(MyFst * t)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.reverse-fn]
> Unimplemented stub. Takes parameter `t` (MyFst*), declared to return
> `MyFst*`. The parameter is discarded via `(void)t` and never used. The
> body then unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer that accepts the reversed string pair (1gnirts:2gnirts)
> whenever `t` accepts string1:string2, but that behavior is not
> implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.substitute-fn]
> MyFst * MyTransducerLibraryTransducer::substitute

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.substitute-fn]
> Unimplemented stub. This is the symbol-to-symbol overload taking `t`
> (MyFst*), `old_symbol` (String), and `new_symbol` (String), declared to
> return `MyFst*`. All three parameters are discarded via `(void)t`,
> `(void)old_symbol`, and `(void)new_symbol` and never used. The body then
> unconditionally executes
> `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a
> transducer equivalent to `t` but with every occurrence of `old_symbol`
> replaced by `new_symbol`, but that behavior is not implemented in this
> template. (The symbol-pair-to-transducer
> `substitute(t, symbol_pair, tr)` overload is a separate stub that
> likewise throws.)

> [spec:hfst:def:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.subtract-fn]
> MyFst * MyTransducerLibraryTransducer::subtract(MyFst * t1, MyFst * t2)

> [spec:hfst:sem:my-transducer-library-transducer.hfst.implementations.my-transducer-library-transducer.subtract-fn]
> Unimplemented stub. Takes parameters `t1` and `t2` (both MyFst*),
> declared to return `MyFst*`. Both parameters are discarded via
> `(void)t1` and `(void)t2` and never used. The body then unconditionally
> executes `HFST_THROW(FunctionNotImplementedException)`, throwing a
> `FunctionNotImplementedException` and never returning a value. No state
> is read or mutated and no allocation occurs. Intended to return a new
> transducer accepting the string pairs accepted by `t1` but not by `t2`
> (set difference), but that behavior is not implemented in this template.

> [spec:hfst:def:my-transducer-library-transducer.main-fn]
> int

> [spec:hfst:sem:my-transducer-library-transducer.main-fn]
> The test-harness entry point compiled only in the `#else` branch (when
> the library is not built with this back-end enabled). Takes no
> parameters, returns int. It writes the literal string
> "Unit tests for " followed by the `__FILE__` macro value and a single ':'
> to std::cout, then writes a newline, the literal "ok", and another
> newline. It performs no actual testing. It then returns `EXIT_SUCCESS`.
> No state is read or mutated beyond standard output.

