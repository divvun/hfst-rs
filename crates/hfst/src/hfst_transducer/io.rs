//! AT&T and prolog text I/O, lexc reading, and tokenizer creation.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: Backend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- AT&T / prolog I/O, tokenizer creation (HfstTransducer.cc ~5823-6410)
    // -------------------------------------------------------------------------
    // 'HfstBasicTransducer net(*this)' is the conversion constructor
    // 'HfstBasicTransducer(const HfstTransducer&)' — ported as
    // 'ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)'.

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
    pub fn write_in_att_format_filename(
        &self,
        filename: &str,
        print_weights: bool,
    ) -> crate::error::Result<()> {
        let file = match std::fs::File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                let message = filename.to_string();
                crate::bail!(StreamCannotBeWritten, message);
            }
        };
        let mut ofile = std::io::BufWriter::new(file);
        self.write_in_att_format_file(&mut ofile, print_weights)
            .and_then(|()| std::io::Write::flush(&mut ofile))
            .map_err(|_| crate::err!(StreamCannotBeWritten, filename))?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
    pub fn write_in_att_format_number(
        &self,
        ofile: &mut dyn std::io::Write,
        print_weights: bool,
    ) -> std::io::Result<()> {
        let net = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        net.write_in_att_format_number_file(ofile, print_weights)
    }

    pub fn write_in_att_format_file(
        &self,
        ofile: &mut dyn std::io::Write,
        print_weights: bool,
    ) -> std::io::Result<()> {
        // Implemented only for internal transducer format.
        let net = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        net.write_in_att_format_file(ofile, print_weights)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
    pub fn write_in_prolog_format(
        &mut self,
        file: &mut dyn std::io::Write,
        name: &str,
        write_weights: bool,
    ) -> crate::error::Result<()> {
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
        fsm.write_in_prolog_format_file(file, name, write_weights)
    }

    /// 'HfstTransducer &read_in_att_format(const std::string &filename, type,
    ///  const std::string &epsilon_symbol, bool warn_negs)'. The target type is
    ///  the type parameter now.
    pub fn read_in_att_format_filename(
        filename: &str,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let ifile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(_) => {
                // [spec:hfst:def:hfst-transducer.hfst.message-fn]
                // [spec:hfst:sem:hfst-transducer.hfst.message-fn]
                crate::bail!(StreamNotReadable, filename);
            }
        };

        let mut reader = std::io::BufReader::new(ifile);
        Self::read_in_att_format_file(&mut reader, epsilon_symbol, warn_negs)
    }

    /// 'HfstTransducer &read_in_att_format(FILE *ifile, type,
    ///  const std::string &epsilon_symbol, bool warn_negs)'.
    pub fn read_in_att_format_file(
        ifile: &mut dyn std::io::BufRead,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut linecount: u32 = 0;
        let net = HfstBasicTransducer::read_in_att_format(
            ifile,
            epsilon_symbol,
            &mut linecount,
            warn_negs,
        )?;
        // C++ 'new HfstTransducer(net, type)' returned a heap pointer the caller
        // owned; the owned value is the idiomatic equivalent.
        let _ = linecount;
        HfstTransducer::new_from_basic(&net)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
    pub fn create_tokenizer(&mut self) -> HfstTokenizer {
        let mut tok = HfstTokenizer::new();

        // (the SFST 'get_symbol_pairs' branch is compiled out with the backend)
        let mut t = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        t.prune_alphabet(true);
        let alpha = t.get_alphabet();
        for it in alpha.iter() {
            if it.len() > 1 {
                tok.add_multichar_symbol(it);
            }
        }

        tok
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
    // The C++ 'type' parameter is the backend type parameter 'B' now
    // ([dec:hfst:monomorphic-backends]); its availability check was pure
    // capability gating and is a static fact of the instantiation.
    pub fn read_lexc(filename: &str, verbose: bool) -> crate::error::Result<HfstTransducer<B>>
    where
        B: AlgebraBackend,
    {
        Ok(HfstTransducer::read_lexc_ptr(filename, verbose)?
            .expect("read_lexc: lexc compilation produced no transducer"))
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
    pub fn read_lexc_ptr(
        filename: &str,
        verbose: bool,
    ) -> crate::error::Result<Option<HfstTransducer<B>>>
    where
        B: AlgebraBackend,
    {
        // The C++ 'compiler.parse(filename.c_str())' reads the file via the
        // Flex/Bison lexer; the ported LexcCompiler walks an AST built from
        // source text instead, so read the file here and feed 'compile'.
        // (The C++ 'new HfstTransducer()' placeholder that it then leaks was a
        // raw-pointer artifact and is gone with the owned return.)
        let mut compiler = crate::lexc::LexcCompiler::<B>::new();
        compiler.set_verbosity(verbose as u32);
        let source = std::fs::read_to_string(filename)
            .map_err(|_| crate::err!(StreamNotReadable, filename))?;
        Ok(compiler.compile(&source))
    }
}

// C++ 'operator<<(std::ostream &out, const HfstTransducer &t)' (HfstTransducer.cc:6419)
// — write the transducer in AT&T format. Implemented only for the internal
// (basic) transducer format: convert to a HfstBasicTransducer and write it.
pub fn write_to<W: std::io::Write, B: Backend>(out: &mut W, t: &HfstTransducer<B>) {
    let net = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(t)
        .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
    // C++ writes weights for every type except SFST/FOMA (both out of scope here).
    let write_weights = t.get_type() != ImplementationType::SFST_TYPE
        && t.get_type() != ImplementationType::FOMA_TYPE;
    net.write_in_att_format_os(out, write_weights);
}
