//! Port of 'libhfst/src/HfstOutputStream.{h,cc}' — 'hfst::HfstOutputStream', the
//! stream for writing binary transducers.
//!
//! ## Monomorphization ([dec:hfst:monomorphic-backends])
//! The C++ held a 'union StreamImplementation' of per-backend output-stream
//! pointers and dispatched every write on the 'type' tag. The per-backend
//! streams were all '{ filename, owned writer, flag }' wrappers whose only
//! per-type behaviour was the payload serialization — which is now
//! 'Backend::write'. So the union collapses to ONE owned writer here; the
//! 'ty' tag survives because an output stream's format genuinely is runtime
//! data (the CLI '--format' value), and it is checked once per transducer
//! against 'Backend::stream_type()' in 'write'. Writing a runtime-typed
//! transducer goes through the one runtime sum: 'AnyTransducer::write'.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use crate::backend::Backend;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_transducer::HfstTransducer;

/// A stream for writing binary transducers.
// [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream]
pub struct HfstOutputStream {
    /// Type of the stream implementation (checked against each written
    /// transducer's 'Backend::stream_type()').
    ty: ImplementationType,
    /// Whether an hfst header is written before every transducer.
    hfst_format: bool,
    /// The single owned byte sink — the collapse of the C++ per-backend
    /// output-stream union ('StreamImplementation'), whose members each owned
    /// an equivalent writer.
    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.stream-implementation]
    out: Box<dyn std::io::Write>,
    /// If file is open.
    is_open: bool,
}

impl HfstOutputStream {
    /// 'HfstOutputStream(ImplementationType type, bool hfst_format=true)' — a
    /// stream to standard output. (No '[spec:]' id in the C++ for this ctor.)
    pub fn new(ty: ImplementationType, hfst_format: bool) -> crate::error::Result<Self> {
        if !crate::hfst_transducer::is_lean_implementation_type_available(ty) {
            crate::bail!(ImplementationTypeNotAvailable(ty));
        }

        match ty {
            ImplementationType::SFST_TYPE => {
                // Unreachable: SFST is excluded by is_lean_implementation_type_available above.
                unreachable!("SFST_TYPE excluded by the availability guard above");
            }
            // Like the openfst/OL backends, foma needs no per-type stream
            // object: operator<< writes the "FOMA" header generically and the
            // payload comes from Backend::write (native .foma). Only reachable
            // with the `foma` feature (availability is gated above).
            ImplementationType::FOMA_TYPE => {}
            ImplementationType::XFSM_TYPE => {
                // Unreachable: XFSM is excluded by is_lean_implementation_type_available above.
                unreachable!("XFSM_TYPE excluded by the availability guard above");
            }
            ImplementationType::TROPICAL_OPENFST_TYPE
            | ImplementationType::HFST_OL_TYPE
            | ImplementationType::HFST_OLW_TYPE => {}
            ImplementationType::HFST2_TYPE
            | ImplementationType::UNSPECIFIED_TYPE
            | ImplementationType::ERROR_TYPE => crate::bail!(SpecifiedTypeRequired),
        }

        Ok(HfstOutputStream {
            ty,
            hfst_format,
            // Raw Stdout is line-buffered: each write takes the lock and scans
            // for newlines, which dominates when streaming big binaries. The
            // C++ FILE* was fully buffered; BufWriter restores that.
            out: Box::new(std::io::BufWriter::new(std::io::stdout())),
            is_open: true,
        })
    }

    // FIXME: HfstOutputStream takes a string parameter,
    //        HfstInputStream a const char*
    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.hfst-output-stream-fn]
    // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.hfst-output-stream-fn]
    /// 'HfstOutputStream(const std::string &filename, ImplementationType type,
    /// bool hfst_format=true)'.
    pub fn new_filename(
        filename: &str,
        ty: ImplementationType,
        hfst_format: bool,
    ) -> crate::error::Result<Self> {
        // The CLI passes the sentinel "<stdout>" (and "") for standard output;
        // route those to the stdout constructor rather than creating a file
        // literally named "<stdout>".
        if filename.is_empty() || filename == "<stdout>" {
            return Self::new(ty, hfst_format);
        }
        if !crate::hfst_transducer::is_lean_implementation_type_available(ty) {
            crate::bail!(ImplementationTypeNotAvailable(ty));
        }

        match ty {
            ImplementationType::SFST_TYPE => {
                // Unreachable: SFST is excluded by is_lean_implementation_type_available above.
                unreachable!("SFST_TYPE excluded by the availability guard above");
            }
            // foma: no per-type stream object; operator<< writes header +
            // Backend::write payload. Only reachable with the `foma` feature.
            ImplementationType::FOMA_TYPE => {}
            ImplementationType::XFSM_TYPE => {
                // Unreachable: XFSM is excluded by is_lean_implementation_type_available above.
                unreachable!("XFSM_TYPE excluded by the availability guard above");
            }
            ImplementationType::TROPICAL_OPENFST_TYPE
            | ImplementationType::HFST_OL_TYPE
            | ImplementationType::HFST_OLW_TYPE => {}
            ImplementationType::HFST2_TYPE
            | ImplementationType::UNSPECIFIED_TYPE
            | ImplementationType::ERROR_TYPE => crate::bail!(SpecifiedTypeRequired),
        }

        // The tropical backend stream panicked on a failed open ('cannot
        // open file'); the failure is now surfaced as a stream error instead.
        let file = match std::fs::File::create(filename) {
            Ok(f) => f,
            Err(_) => crate::bail!(StreamCannotBeWritten, filename),
        };

        Ok(HfstOutputStream {
            ty,
            hfst_format,
            // Unbuffered File = one syscall per write; the C++ FILE* buffered.
            out: Box::new(std::io::BufWriter::new(file)),
            is_open: true,
        })
    }

    // '~HfstOutputStream' (C++) just 'delete's the active backend pointer; the
    // owned writer is freed automatically when the struct is dropped, so no
    // explicit 'Drop' impl is required.

    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-fn]
    // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-fn]
    fn append(str: &mut Vec<char>, s: &str) {
        for b in s.as_bytes() {
            str.push(*b as char);
        }
        str.push('\0');
    }

    fn write_string(&mut self, s: &str) {
        for b in s.as_bytes() {
            self.write_char(*b as char);
        }
    }

    fn write_char_vector(&mut self, s: &Vec<char>) {
        for i in 0..s.len() {
            self.write_char(s[i]);
        }
    }

    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.write-fn]
    // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.write-fn]
    fn write_char(&mut self, c: char) {
        // The C++ dispatched this byte write to the active backend stream;
        // every in-scope backend just wrote to its owned writer.
        if self.out.write_all(&[c as u8]).is_err() {
            tracing::error!("HfstOutputStream: could not write a byte");
        }
    }

    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-hfst-header-data-fn]
    // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-hfst-header-data-fn]
    fn append_hfst_header_data(&mut self, header: &mut Vec<char>) {
        Self::append(header, "version");
        Self::append(header, "3.3");
        Self::append(header, "type");

        let type_value: String = match self.ty {
            ImplementationType::SFST_TYPE => "SFST".to_string(),
            ImplementationType::TROPICAL_OPENFST_TYPE => "TROPICAL_OPENFST".to_string(),
            ImplementationType::FOMA_TYPE => "FOMA".to_string(),
            ImplementationType::XFSM_TYPE => "XFSM".to_string(),
            ImplementationType::HFST_OL_TYPE => "HFST_OL".to_string(),
            ImplementationType::HFST_OLW_TYPE => "HFST_OLW".to_string(),
            ImplementationType::HFST2_TYPE
            | ImplementationType::UNSPECIFIED_TYPE
            | ImplementationType::ERROR_TYPE => {
                assert!(false);
                String::new()
            }
        };

        Self::append(header, &type_value);
    }

    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-implementation-specific-header-data-fn]
    // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-implementation-specific-header-data-fn]
    // Only the SFST backend contributed implementation-specific header data
    // (its alphabet); every in-scope type is a no-op, so this stays for shape.
    fn append_implementation_specific_header_data(&mut self, header: &mut Vec<char>) {
        let _ = header;
    }

    pub fn flush(&mut self) -> crate::error::Result<&mut Self> {
        if !self.is_open {
            crate::bail!(StreamIsClosed);
        }
        // (The XFSM deferred-write flush is compiled out with the backend.)
        Ok(self)
    }

    /// An alias for 'operator<<'.
    pub fn redirect<B: Backend>(
        &mut self,
        transducer: &mut HfstTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.write(transducer)
    }

    /// 'HfstOutputStream &operator<< (HfstTransducer &transducer)'.
    pub fn write<B: Backend>(
        &mut self,
        transducer: &mut HfstTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        if !self.is_open {
            crate::bail!(StreamIsClosed);
        }

        if self.ty != transducer.fst.stream_type() {
            crate::bail!(
                TransducerTypeMismatch,
                "operator<<: HfstOutputStream and HfstTransducer do not have the same type"
            );
        }

        /* Write the HFST header. The header has the following structure:

           - the first four chars identify an HFST header:  "HFST"
           - the fifth char is a separator:                 "\0"
           - the sixth and seventh char tell the length of the rest of the header
             (beginning after the eighth char)
           - the eighth char is a separator and is not counted
             to the header length: "\0"
           - the rest of the header consists of pairs of attributes and their values
             that are each separated by a char "\0"

           HFST version 3.0 header must contain at least the attributes 'version',
           'type' and 'name' and their values. Implementation-specific attributes
           can follow after these obligatory attributes.

           Note: in XFSM format, we never write the HFST header. hfst_format is always
           false if the stream is of XFSM format.
        */
        if self.hfst_format {
            const MAX_HEADER_LENGTH: i32 = 65535;

            // collect the header data here
            let mut header: Vec<char> = Vec::new();
            self.append_hfst_header_data(&mut header); // attributes "version" and "type"
            for (key, value) in transducer.get_properties().iter() {
                if key.as_str() == "type" || key.as_str() == "version" {
                    // special hanling above
                    continue;
                }
                Self::append(&mut header, key.as_str());
                Self::append(&mut header, value.as_str());
            }

            self.append_implementation_specific_header_data(&mut header);

            // write the field that identifies the header as an HFST header
            self.write_string("HFST");
            self.write_char('\0');

            // write header length using two bytes
            let header_length: i32 = header.len() as i32;
            if header_length > MAX_HEADER_LENGTH {
                crate::bail!(Fatal, "transducer header is too long");
            }

            // mirrors C++ '*((char*)(&header_length)+i)' (native-endian byte punning)
            let header_length_bytes = header_length.to_ne_bytes();
            let first_byte = header_length_bytes[0] as char;
            let second_byte = header_length_bytes[1] as char;
            self.write_char(first_byte);
            self.write_char(second_byte);
            self.write_char('\0');

            // write the rest of the header
            self.write_char_vector(&header);
        } // if (hfst_format)

        // The per-type 'implementation.X->write_transducer(...)' dispatch is
        // the backend's own payload serialization now.
        transducer.fst.write(&mut *self.out, self.hfst_format)?;
        if self.out.flush().is_err() {
            crate::bail!(StreamCannotBeWritten, "could not flush output stream");
        }
        Ok(self)
    }

    // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.close-fn]
    // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.close-fn]
    pub fn close(&mut self) {
        // Flush unconditionally: stdout is a buffered std::io::stdout() and the
        // tools exit via std::process::exit (no Drop), so it must be flushed.
        if self.out.flush().is_err() {
            tracing::error!("HfstOutputStream: could not flush output stream on close");
        }
        self.is_open = false;
    }

    /// The stream's format tag (the CLI '--format' value it was built with).
    pub fn get_type(&self) -> ImplementationType {
        self.ty
    }
}
