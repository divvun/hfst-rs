//! Port of 'libhfst/src/HfstOutputStream.{h,cc}' — 'hfst::HfstOutputStream', the
//! stream for writing binary transducers.
//!
//! ## Backend union modelling
//! The C++ holds a 'union StreamImplementation' of raw backend-stream pointers
//! ('log_ofst', 'tropical_ofst', 'sfst', 'foma', 'xfsm', 'hfst_ol'); exactly one
//! member is live, selected by the 'type' ('ImplementationType') discriminant.
//! Mirrored here as a struct of per-backend 'Option<Box<...>>' fields
//! (['StreamImplementation']). The tropical/log output streams own their writer
//! (no lifetime), so 'HfstOutputStream' needs no lifetime parameter.
//! 'sfst'/'foma'/'xfsm' backend streams are unported collaborators — placeholder
//! unit types so the field exists but cannot be constructed. 'hfst_ol' uses the
//! real ['HfstOlOutputStream'] from 'hfst_ol_transducer'.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]

use crate::hfst_data_types::ImplementationType;
use crate::hfst_ol_transducer::HfstOlOutputStream;
use crate::log_weight_transducer::LogWeightOutputStream;
use crate::tropical_weight_transducer::TropicalWeightOutputStream;

/// Unported backend output-stream collaborators. Placeholder unit types: the
/// corresponding ['StreamImplementation'] field exists for fidelity with the C++
/// union, but no constructor is provided (the '#if HAVE_SFST' / 'HAVE_FOMA' /
/// 'HAVE_XFSM' paths are deferred).
pub struct SfstOutputStream;
pub struct FomaOutputStream;
pub struct XfsmOutputStream;

/// Port of the C++ 'union StreamImplementation' (backend implementation). Exactly
/// one field is 'Some', selected by 'HfstOutputStream::type_'.
// [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.stream-implementation]
#[derive(Default)]
pub struct StreamImplementation {
    pub log_ofst: Option<Box<LogWeightOutputStream>>,
    pub tropical_ofst: Option<Box<TropicalWeightOutputStream>>,
    pub sfst: Option<Box<SfstOutputStream>>,
    pub foma: Option<Box<FomaOutputStream>>,
    pub xfsm: Option<Box<XfsmOutputStream>>,
    pub hfst_ol: Option<Box<HfstOlOutputStream>>,
}

/// A stream for writing binary transducers.
// [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream]
pub struct HfstOutputStream {
    /// Type of the stream implementation (the discriminant selecting the live
    /// 'implementation' member).
    type_: ImplementationType,
    /// Whether an hfst header is written before every transducer.
    hfst_format: bool,
    /// Backend implementation (C++ 'StreamImplementation implementation').
    implementation: StreamImplementation,
    /// If file is open.
    is_open: bool,
}

// ===== output_impl (workflow body) =====
mod output_impl {
    #![allow(unused_imports)]
    use super::*;
    use super::*;

    use crate::hfst_data_types::ImplementationType;
    use crate::hfst_transducer::HfstTransducer;
    use crate::log_weight_transducer::LogWeightOutputStream;
    use crate::tropical_weight_transducer::TropicalWeightOutputStream;

    #[allow(dead_code)]
    impl HfstOutputStream {
        /// 'HfstOutputStream(ImplementationType type, bool hfst_format=true)' — a
        /// stream to standard output. (No '[spec:]' id in the C++ for this ctor.)
        pub fn new(type_: ImplementationType, hfst_format: bool) -> crate::error::Result<Self> {
            if !HfstTransducer::is_lean_implementation_type_available(type_) {
                // 'throw ImplementationTypeNotAvailableException(...)' — a direct
                // panic_any rather than HFST_THROW (which can't carry the 'type_').
                crate::bail!(ImplementationTypeNotAvailable(type_));
            }

            let mut implementation = StreamImplementation {
                sfst: None,
                tropical_ofst: None,
                log_ofst: None,
                foma: None,
                xfsm: None,
                hfst_ol: None,
            };

            match type_ {
                ImplementationType::SFST_TYPE => {
                    // implementation.sfst = new hfst::implementations::SfstOutputStream();
                    unimplemented!("deferred: SfstOutputStream");
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    implementation.tropical_ofst =
                        Some(Box::new(TropicalWeightOutputStream::new(hfst_format)));
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    implementation.log_ofst = Some(Box::new(LogWeightOutputStream::new()));
                }
                ImplementationType::FOMA_TYPE => {
                    // implementation.foma = new hfst::implementations::FomaOutputStream();
                    unimplemented!("deferred: FomaOutputStream");
                }
                ImplementationType::XFSM_TYPE => {
                    // implementation.xfsm = new XfsmOutputStream(); // throws error, not implemented
                    unimplemented!("deferred: XfsmOutputStream");
                }
                ImplementationType::HFST_OL_TYPE => {
                    implementation.hfst_ol = Some(Box::new(HfstOlOutputStream::new(false)));
                }
                ImplementationType::HFST_OLW_TYPE => {
                    implementation.hfst_ol = Some(Box::new(HfstOlOutputStream::new(true)));
                }
                _ => crate::bail!(SpecifiedTypeRequired),
            }

            Ok(HfstOutputStream {
                type_,
                hfst_format,
                implementation,
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
            type_: ImplementationType,
            hfst_format: bool,
        ) -> crate::error::Result<Self> {
            // The CLI passes the sentinel "<stdout>" (and "") for standard output;
            // route those to the stdout constructor rather than creating a file
            // literally named "<stdout>".
            if filename.is_empty() || filename == "<stdout>" {
                return Self::new(type_, hfst_format);
            }
            if !HfstTransducer::is_lean_implementation_type_available(type_) {
                crate::bail!(ImplementationTypeNotAvailable(type_));
            }

            let mut implementation = StreamImplementation {
                sfst: None,
                tropical_ofst: None,
                log_ofst: None,
                foma: None,
                xfsm: None,
                hfst_ol: None,
            };

            match type_ {
                ImplementationType::SFST_TYPE => {
                    // implementation.sfst = new SfstOutputStream(filename);
                    unimplemented!("deferred: SfstOutputStream");
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    // FIXME: this should be done in TropicalWeight layer
                    if filename.is_empty() {
                        implementation.tropical_ofst =
                            Some(Box::new(TropicalWeightOutputStream::new(hfst_format)));
                    } else {
                        implementation.tropical_ofst = Some(Box::new(
                            TropicalWeightOutputStream::new_filename(filename, hfst_format),
                        ));
                    }
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    implementation.log_ofst =
                        Some(Box::new(LogWeightOutputStream::new_filename(filename)));
                }
                ImplementationType::FOMA_TYPE => {
                    // implementation.foma = new FomaOutputStream(filename);
                    unimplemented!("deferred: FomaOutputStream");
                }
                ImplementationType::XFSM_TYPE => {
                    // XFSM api only offers a function that reads transducers that takes a
                    // filename argument. That is why we don't write an HFST header:
                    //     hfst_format = false;
                    //     implementation.xfsm = new XfsmOutputStream(filename);
                    unimplemented!("deferred: XfsmOutputStream");
                }
                ImplementationType::HFST_OL_TYPE => {
                    implementation.hfst_ol =
                        Some(Box::new(HfstOlOutputStream::new_filename(filename, false)));
                }
                ImplementationType::HFST_OLW_TYPE => {
                    implementation.hfst_ol =
                        Some(Box::new(HfstOlOutputStream::new_filename(filename, true)));
                }
                _ => crate::bail!(SpecifiedTypeRequired),
            }

            Ok(HfstOutputStream {
                type_,
                hfst_format,
                implementation,
                is_open: true,
            })
        }

        // '~HfstOutputStream' (C++) just 'delete's the active backend pointer; in Rust
        // each owned 'Option<Box<…>>' backend is freed automatically when the struct is
        // dropped, so no explicit 'Drop' impl is required.

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
            match self.type_ {
                ImplementationType::SFST_TYPE => unimplemented!("deferred: SfstOutputStream"),
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation.tropical_ofst.as_mut().unwrap().write(c);
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_mut().unwrap().write(c);
                }
                ImplementationType::FOMA_TYPE => unimplemented!("deferred: FomaOutputStream"),
                ImplementationType::XFSM_TYPE => {
                    std::panic::panic_any(
                        "operation XfsmOutputStream::write(const char &c) not supported",
                    );
                }
                // we always have HFST_OL, right?
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    self.implementation.hfst_ol.as_mut().unwrap().write(c);
                }
                _ => {
                    assert!(false);
                }
            }
        }

        // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-hfst-header-data-fn]
        // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-hfst-header-data-fn]
        fn append_hfst_header_data(&mut self, header: &mut Vec<char>) {
            Self::append(header, "version");
            Self::append(header, "3.3");
            Self::append(header, "type");

            let type_value: String = match self.type_ {
                ImplementationType::SFST_TYPE => "SFST".to_string(),
                ImplementationType::TROPICAL_OPENFST_TYPE => "TROPICAL_OPENFST".to_string(),
                ImplementationType::LOG_OPENFST_TYPE => "LOG_OPENFST".to_string(),
                ImplementationType::FOMA_TYPE => "FOMA".to_string(),
                ImplementationType::XFSM_TYPE => "XFSM".to_string(),
                ImplementationType::HFST_OL_TYPE => "HFST_OL".to_string(),
                ImplementationType::HFST_OLW_TYPE => "HFST_OLW".to_string(),
                _ => {
                    assert!(false);
                    String::new()
                }
            };

            Self::append(header, &type_value);
        }

        // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-implementation-specific-header-data-fn]
        // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-implementation-specific-header-data-fn]
        #[allow(unused_variables)]
        fn append_implementation_specific_header_data(
            &mut self,
            header: &mut Vec<char>,
            transducer: &mut HfstTransducer,
        ) {
            // HAVE_SFST/HAVE_LEAN_SFST branch: only SFST contributes implementation-specific
            // header data (its alphabet); every other type is a no-op.
            match self.type_ {
                ImplementationType::SFST_TYPE => {
                    // implementation.sfst->append_implementation_specific_header_data(
                    //     header, transducer.implementation.sfst);
                    unimplemented!("deferred: SfstOutputStream");
                }
                _ => {}
            }
        }

        pub fn flush(&mut self) -> crate::error::Result<&mut Self> {
            if !self.is_open {
                crate::bail!(StreamIsClosed);
            }
            if self.type_ == ImplementationType::XFSM_TYPE {
                // implementation.xfsm->flush();
                unimplemented!("deferred: XfsmOutputStream");
            }
            Ok(self)
        }

        /// An alias for 'operator<<'.
        pub fn redirect(
            &mut self,
            transducer: &mut HfstTransducer,
        ) -> crate::error::Result<&mut Self> {
            self.operator_shl(transducer)
        }

        /// 'HfstOutputStream &operator<< (HfstTransducer &transducer)'.
        pub fn operator_shl(
            &mut self,
            transducer: &mut HfstTransducer,
        ) -> crate::error::Result<&mut Self> {
            if !self.is_open {
                crate::bail!(StreamIsClosed);
            }

            if self.type_ != transducer.type_ {
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
                for (key, value) in transducer.props.iter() {
                    if key.as_str() == "type" || key.as_str() == "version" {
                        // special hanling above
                        continue;
                    }
                    Self::append(&mut header, key.as_str());
                    Self::append(&mut header, value.as_str());
                }

                self.append_implementation_specific_header_data(&mut header, transducer);

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

            Ok(match self.type_ {
                ImplementationType::SFST_TYPE => {
                    // implementation.sfst->write_transducer(transducer.implementation.sfst);
                    unimplemented!("deferred: SfstOutputStream")
                }
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation
                        .tropical_ofst
                        .as_mut()
                        .unwrap()
                        .write_transducer(transducer.implementation.as_tropical());
                    self
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation
                        .log_ofst
                        .as_mut()
                        .unwrap()
                        .write_transducer(transducer.implementation.as_log());
                    self
                }
                ImplementationType::FOMA_TYPE => {
                    // implementation.foma->write_transducer(transducer.implementation.foma);
                    unimplemented!("deferred: FomaOutputStream")
                }
                // This stores the transducer in a list that is written only when flush() is called.
                ImplementationType::XFSM_TYPE => {
                    // implementation.xfsm->write_transducer(transducer.implementation.xfsm);
                    unimplemented!("deferred: XfsmOutputStream")
                }
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    self.implementation
                        .hfst_ol
                        .as_mut()
                        .unwrap()
                        .write_transducer(transducer.implementation.as_hfst_ol());
                    self
                }
                _ => {
                    assert!(false);
                    self
                }
            })
        }

        // [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.close-fn]
        // [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.close-fn]
        pub fn close(&mut self) {
            match self.type_ {
                ImplementationType::SFST_TYPE => unimplemented!("deferred: SfstOutputStream"),
                ImplementationType::TROPICAL_OPENFST_TYPE => {
                    self.implementation.tropical_ofst.as_mut().unwrap().close();
                }
                ImplementationType::LOG_OPENFST_TYPE => {
                    self.implementation.log_ofst.as_mut().unwrap().close();
                }
                ImplementationType::FOMA_TYPE => unimplemented!("deferred: FomaOutputStream"),
                ImplementationType::XFSM_TYPE => unimplemented!("deferred: XfsmOutputStream"),
                ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                    self.implementation.hfst_ol.as_mut().unwrap().close();
                }
                _ => {
                    assert!(false);
                }
            }
            self.is_open = false;
        }
    }
}
