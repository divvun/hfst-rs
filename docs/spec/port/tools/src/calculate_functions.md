# tools/src/calculate_functions.h

> [spec:hfst:def:calculate-functions.add-epsilon-to-alphabet-fn]
> void add_epsilon_to_alphabet()

> [spec:hfst:sem:calculate-functions.add-epsilon-to-alphabet-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.add-range-fn]
> Ranges *add_range( Range *r, Ranges *rs )

> [spec:hfst:sem:calculate-functions.add-range-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.add-value-fn]
> Range *add_value( Symbol c, Range *r )

> [spec:hfst:sem:calculate-functions.add-value-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.add-values-fn]
> Range *add_values( unsigned int c1, unsigned int c2, Range *r )

> [spec:hfst:sem:calculate-functions.add-values-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.add-var-values-fn]
> Range *add_var_values( char *name, Range *r )

> [spec:hfst:sem:calculate-functions.add-var-values-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.character-code-fn]
> Key character_code( unsigned int c )

> [spec:hfst:sem:calculate-functions.character-code-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.complement-range-fn]
> Range *complement_range( Range *r )

> [spec:hfst:sem:calculate-functions.complement-range-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.copy-range-agreement-variable-value-fn]
> Range *copy_range_agreement_variable_value( char *name )

> [spec:hfst:sem:calculate-functions.copy-range-agreement-variable-value-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.copy-range-variable-value-fn]
> Range *copy_range_variable_value( char *name )

> [spec:hfst:sem:calculate-functions.copy-range-variable-value-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.copy-transducer-agreement-variable-value-fn]
> TransducerHandle copy_transducer_agreement_variable_value( char *name )

> [spec:hfst:sem:calculate-functions.copy-transducer-agreement-variable-value-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.copy-transducer-variable-value-fn]
> TransducerHandle copy_transducer_variable_value( char *name )

> [spec:hfst:sem:calculate-functions.copy-transducer-variable-value-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.define-range-variable-fn]
> bool define_range_variable( char *name, Range *r )

> [spec:hfst:sem:calculate-functions.define-range-variable-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.define-transducer-agreement-variable-fn]
> bool define_transducer_agreement_variable( char *name, TransducerHandle t )

> [spec:hfst:sem:calculate-functions.define-transducer-agreement-variable-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.define-transducer-variable-fn]
> bool define_transducer_variable( char *name, TransducerHandle t )

> [spec:hfst:sem:calculate-functions.define-transducer-variable-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.error2-fn]
> void error2( char *message, char *input )

> [spec:hfst:sem:calculate-functions.error2-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.explode-and-minimise-fn]
> TransducerHandle explode_and_minimise(TransducerHandle t)

> [spec:hfst:sem:calculate-functions.explode-and-minimise-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.explode-fn]
> TransducerHandle explode( TransducerHandle t )

> [spec:hfst:sem:calculate-functions.explode-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.insert-freely-fn]
> TransducerHandle insert_freely( TransducerHandle t, Key k1, Key k2 )

> [spec:hfst:sem:calculate-functions.insert-freely-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.make-mapping-fn]
> TransducerHandle make_mapping( Ranges *rs1, Ranges *rs2 )

> [spec:hfst:sem:calculate-functions.make-mapping-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.make-rule-fn]
> TransducerHandle make_rule(TransducerHandle t1, Range *r1, Twol_Type type, Range *r2, TransducerHandle t2, KeyPairSet *Pi)

> [spec:hfst:sem:calculate-functions.make-rule-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.new-transducer-fn]
> TransducerHandle new_transducer( Range *r1, Range *r2, KeyPairSet *Pi )

> [spec:hfst:sem:calculate-functions.new-transducer-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.read-symbol-table-text-fn]
> void read_symbol_table_text(istream& is)

> [spec:hfst:sem:calculate-functions.read-symbol-table-text-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.read-transducer-and-harmonize-fn]
> TransducerHandle read_transducer_and_harmonize( char *filename )

> [spec:hfst:sem:calculate-functions.read-transducer-and-harmonize-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.read-transducer-text-fn]
> TransducerHandle read_transducer_text( char *filename, KeyTable *T, bool sfst=false )

> [spec:hfst:sem:calculate-functions.read-transducer-text-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.read-words-fn]
> TransducerHandle read_words( char* filename )

> [spec:hfst:sem:calculate-functions.read-words-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.result-fn]
> TransducerHandle result( TransducerHandle t, bool switch_flag, bool reverse=false )

> [spec:hfst:sem:calculate-functions.result-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.set-alphabet-defined-fn]
> void set_alphabet_defined( int i )

> [spec:hfst:sem:calculate-functions.set-alphabet-defined-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.symbol-code-fn]
> Key symbol_code( char *s )

> [spec:hfst:sem:calculate-functions.symbol-code-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.utf8-to-int-fn]
> unsigned int utf8_to_int( char *s )

> [spec:hfst:sem:calculate-functions.utf8-to-int-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:calculate-functions.write-to-file-fn]
> void write_to_file( TransducerHandle t )

> [spec:hfst:sem:calculate-functions.write-to-file-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

