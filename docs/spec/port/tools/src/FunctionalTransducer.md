# tools/src/FunctionalTransducer.cc, tools/src/FunctionalTransducer.h

> [spec:hfst:def:functional-transducer.functional-transducer]
> class FunctionalTransducer {
>   static std::ostream * verbose_out;
>   const HfstBasicTransducer &fst;
>   HfstTokenizer input_tokenizer;
>   HfstTokenizer output_tokenizer;
> }

> [spec:hfst:def:functional-transducer.functional-transducer.apply-fn]
> StringVector

> [spec:hfst:sem:functional-transducer.functional-transducer.apply-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.apply-on-input-fn]
> std::string FunctionalTransducer::apply_on_input(const std::string &input)

> [spec:hfst:sem:functional-transducer.functional-transducer.apply-on-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.apply-on-output-fn]
> std::string FunctionalTransducer::apply_on_output(const std::string &output)

> [spec:hfst:sem:functional-transducer.functional-transducer.apply-on-output-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.collect-symbols-from-fst-fn]
> void FunctionalTransducer::collect_symbols_from_fst

> [spec:hfst:sem:functional-transducer.functional-transducer.collect-symbols-from-fst-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.define-multichar-symbols-fn]
> void FunctionalTransducer::define_multichar_symbols

> [spec:hfst:sem:functional-transducer.functional-transducer.define-multichar-symbols-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.follow-epsilon-transitions-fn]
> void follow_epsilon_transitions

> [spec:hfst:sem:functional-transducer.functional-transducer.follow-epsilon-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.follow-transitions-fn]
> void follow_transitions

> [spec:hfst:sem:functional-transducer.functional-transducer.follow-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.functional-transducer-fn]
> FunctionalTransducer::FunctionalTransducer(const HfstBasicTransducer &fst)

> [spec:hfst:sem:functional-transducer.functional-transducer.functional-transducer-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.join-and-uniquify-fn]
> void join_and_uniquify(StringVectorVector &)

> [spec:hfst:sem:functional-transducer.functional-transducer.join-and-uniquify-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.join-fn]
> std::string FunctionalTransducer::join(const StringVector & v,

> [spec:hfst:sem:functional-transducer.functional-transducer.join-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.set-verbose-print-fn]
> void FunctionalTransducer::set_verbose_print(std::ostream &out)

> [spec:hfst:sem:functional-transducer.functional-transducer.set-verbose-print-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.test-on-pair-string-fn]
> bool test_on_pair_string(const std::string &,const std::string &) const

> [spec:hfst:sem:functional-transducer.functional-transducer.test-on-pair-string-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.test-on-string-pair-fn]
> bool test_on_string_pair(const std::string &,const std::string &) const

> [spec:hfst:sem:functional-transducer.functional-transducer.test-on-string-pair-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.tokenize-fn]
> StringVector FunctionalTransducer::tokenize

> [spec:hfst:sem:functional-transducer.functional-transducer.tokenize-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.functional-transducer.verbose-print-fn]
> void FunctionalTransducer::verbose_print(const std::string &msg)

> [spec:hfst:sem:functional-transducer.functional-transducer.verbose-print-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.main-fn]
> int main(void)

> [spec:hfst:sem:functional-transducer.main-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:functional-transducer.string-set]
> typedef std::set<std::string> StringSet

> [spec:hfst:def:functional-transducer.transition-set]
> typedef std::set<HfstBasicTransition> TransitionSet

