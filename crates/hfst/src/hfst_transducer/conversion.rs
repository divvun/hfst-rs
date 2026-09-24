//! Typed backend conversions and the stream-boundary sum [`AnyTransducer`].

use super::*;

impl<B: Backend> HfstTransducer<B> {
    /// Convert the backend to a weighted optimized-lookup transducer — the
    /// pmatch archive writer's per-member conversion ([`Backend::to_hfst_ol`];
    /// facade metadata is not carried over, the caller re-applies name and
    /// properties on the wrapped result).
    pub fn to_hfst_ol(
        &self,
        weighted: bool,
        options: &str,
        harmonizer: Option<&crate::transducer::Transducer>,
    ) -> crate::error::Result<crate::transducer::Transducer> {
        self.fst.to_hfst_ol(weighted, options, harmonizer)
    }

    // -------------------------------------------------------------------------
    // ----- Conversion functions (typed; the runtime 'convert(ty)' is gone) -----
    // -------------------------------------------------------------------------

    /// The typed conversion to the interchange transducer
    /// ([dec:hfst:monomorphic-backends]); cross-backend conversion is
    /// 'HfstTransducer::<Target>::new_from_basic(&t.to_basic()?)'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
    pub fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        self.fst.to_basic()
    }

    /// For internal use: create an 'HfstBasicTransducer' equivalent to '*this'
    /// and delete the backend implementation.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
    pub fn convert_to_basic_transducer(&mut self) -> crate::error::Result<HfstBasicTransducer> {
        let net = self.fst.to_basic()?;
        // The C++ 'delete's the backend here and leaves a null pointer until
        // 'convert_to_hfst_transducer' restores it; an empty backend stands in
        // for the null (a facade without a backend is not representable).
        self.fst = B::empty();
        Ok(net)
    }

    /// For internal use: build a backend equivalent to 't', delete 't', and
    /// store it as this transducer's implementation.
    pub fn convert_to_hfst_transducer(
        &mut self,
        t: HfstBasicTransducer,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.name = t.name.clone();
        self.fst = B::from_basic_owned(t)?;
        Ok(self)
    }
}

impl<B: AlgebraBackend> HfstTransducer<B> {
    /// The typed algebra->OL conversion of [dec:hfst:monomorphic-backends]
    /// (the C++ 'convert(HFST_OLW_TYPE)' / 'convert(HFST_OL_TYPE)' pair).
    /// Both build weighted-shaped tables in memory, exactly as the C++ did
    /// even for HFST_OL_TYPE output; 'weighted' only sets the header flag,
    /// i.e. the stream type the result serializes under. 'options' is the
    /// C++ convert's options string ("quick" skips the hard table packing).
    /// The facade metadata survives, as it did through the C++ convert.
    pub fn to_ol(
        &self,
        weighted: bool,
        options: &str,
    ) -> crate::error::Result<HfstTransducer<Transducer<WeightedTables>>> {
        let net = self.to_basic()?;
        let ol = crate::convert_transducer_format::ConversionFunctions::
            hfst_basic_transducer_to_hfst_ol(&net, weighted, options, None)?;
        let mut t = HfstTransducer::wrap(ol);
        t.name = self.name.clone();
        t.props = self.props.clone();
        t.anonymous = self.anonymous;
        t.is_trie = self.is_trie;
        Ok(t)
    }

    /// Convert to a native foma transducer. The runtime-type analogue of the
    /// FOMA_TYPE arm of C++ `HfstTransducer::convert`: go through the basic
    /// transducer (`hfst_basic_transducer_to_foma`) and wrap the result. Used by
    /// the CLI to write a compiled (algebra-backend) transducer to a foma stream.
    #[cfg(feature = "foma")]
    pub fn to_foma(
        &self,
    ) -> crate::error::Result<HfstTransducer<crate::backend_foma::FomaTransducer>> {
        let net = self.to_basic()?;
        let foma =
            <crate::backend_foma::FomaTransducer as crate::backend::Backend>::from_basic(&net)?;
        let mut t = HfstTransducer::wrap(foma);
        t.name = self.name.clone();
        t.props = self.props.clone();
        t.anonymous = self.anonymous;
        t.is_trie = self.is_trie;
        Ok(t)
    }
}

// -----------------------------------------------------------------------------
// THFST <-> OLW cheap conversions — O(1) table MOVES that transfer the inner
// weighted optimized-lookup engine and preserve the facade metadata (the inner
// 'fst' is 'pub(crate)', so these must live in the hfst crate).
// -----------------------------------------------------------------------------

impl HfstTransducer<Transducer<WeightedTables>> {
    /// Re-tag this weighted optimized-lookup transducer as THFST — an O(1)
    /// table move, not a round-trip through the basic transducer; the facade
    /// metadata survives.
    // [spec:hfst:def:thfst-backend.olw-moves]
    // [spec:hfst:sem:thfst-backend.olw-moves]
    pub fn into_thfst(self) -> HfstTransducer<crate::backend_thfst::ThfstTransducer> {
        rewrap_facade(self, crate::backend_thfst::ThfstTransducer)
    }
}

impl HfstTransducer<crate::backend_thfst::ThfstTransducer> {
    /// Re-tag this THFST transducer as weighted optimized-lookup — the inverse
    /// O(1) table move; the facade metadata survives.
    pub fn into_olw(self) -> HfstTransducer<Transducer<WeightedTables>> {
        rewrap_facade(self, |b| b.into_ol())
    }
}

// -----------------------------------------------------------------------------
// The one runtime sum ([dec:hfst:monomorphic-backends]).
// -----------------------------------------------------------------------------

/// The single runtime type sum, produced ONLY by the stream readers
/// ('HfstInputStream::read') — the point where file bytes (whose type is data,
/// not code) enter the program. It replaces the C++ union port
/// 'TransducerImplementation' at the stream boundary; everywhere else the
/// backend is the type parameter of ['HfstTransducer'].
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.transducer-implementation]
pub enum AnyTransducer {
    Tropical(HfstTransducer<StdVectorFst>),
    OlW(HfstTransducer<Transducer<WeightedTables>>),
    OlU(HfstTransducer<Transducer<UnweightedTables>>),
    #[cfg(feature = "foma")]
    Foma(HfstTransducer<crate::backend_foma::FomaTransducer>),
    Thfst(HfstTransducer<crate::backend_thfst::ThfstTransducer>),
}

/// Delegate an expression over every variant (each arm monomorphizes
/// separately).
macro_rules! any_delegate {
    ($any:expr, $t:ident => $body:expr) => {
        match $any {
            AnyTransducer::Tropical($t) => $body,
            AnyTransducer::OlW($t) => $body,
            AnyTransducer::OlU($t) => $body,
            #[cfg(feature = "foma")]
            AnyTransducer::Foma($t) => $body,
            AnyTransducer::Thfst($t) => $body,
        }
    };
}

impl AnyTransducer {
    /// The stream/serialization tag: 'Backend::TYPE', except that the OL
    /// backends carry the logical OL/OLW distinction in the payload header
    /// (interim invariant: in-memory OL tables are always weighted-shaped).
    pub fn get_type(&self) -> ImplementationType {
        any_delegate!(self, t => t.fst.stream_type())
    }

    pub fn get_name(&self) -> String {
        any_delegate!(self, t => t.get_name())
    }

    pub fn set_name(&mut self, name: &str) {
        any_delegate!(self, t => t.set_name(name))
    }

    pub fn get_property(&self, property: &str) -> String {
        any_delegate!(self, t => t.get_property(property))
    }

    pub fn set_property(&mut self, property: &str, name: &str) {
        any_delegate!(self, t => t.set_property(property, name))
    }

    pub fn get_properties(&self) -> &BTreeMap<String, String> {
        any_delegate!(self, t => t.get_properties())
    }

    /// The typed conversion to the interchange transducer.
    pub fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        any_delegate!(self, t => t.to_basic())
    }

    /// Write this transducer to an HFST output stream ('operator<<'); the
    /// stream's per-type logic collapses onto this single dispatch.
    pub fn write(
        &mut self,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> crate::error::Result<()> {
        any_delegate!(self, t => { out.write(t)?; Ok(()) })
    }

    /// Typed extraction from the stream sum — the C++ pattern
    /// 'HfstTransducer t(instream); t.convert(format);' of the compilers'
    /// '@bin' file loads. The matching variant moves out unchanged; any other
    /// variant converts through the interchange transducer (a typed
    /// 'convert', [dec:hfst:monomorphic-backends]), preserving the facade
    /// metadata as the C++ convert did.
    pub fn into_typed<B: FromAnyTransducer>(self) -> crate::error::Result<HfstTransducer<B>> {
        B::from_any(self)
    }
}

/// The per-backend arm of ['AnyTransducer::into_typed']: each backend takes
/// its own variant by move and converts the rest via the interchange
/// transducer.
pub trait FromAnyTransducer: Backend {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>>;
}

/// Re-tag a facade around a transformed backend, preserving the metadata
/// (name, properties, anonymous, is_trie). The O(1)-move analogue of
/// ['any_into_backend_via_basic']: the backend is transferred by 'f', not
/// rebuilt through the interchange transducer.
/// [spec:hfst:def:thfst-backend.olw-moves]
/// [spec:hfst:sem:thfst-backend.olw-moves]
fn rewrap_facade<A: Backend, B: Backend>(
    src: HfstTransducer<A>,
    f: impl FnOnce(A) -> B,
) -> HfstTransducer<B> {
    HfstTransducer {
        name: src.name,
        props: src.props,
        anonymous: src.anonymous,
        is_trie: src.is_trie,
        fst: f(src.fst),
    }
}

/// The convert-through-basic arm of ['AnyTransducer::into_typed'].
fn any_into_backend_via_basic<B: Backend>(
    any: AnyTransducer,
) -> crate::error::Result<HfstTransducer<B>> {
    let net = any.to_basic()?;
    let mut t: HfstTransducer<B> = HfstTransducer::wrap(B::from_basic(&net)?);
    // The C++ convert replaced only the implementation; the facade metadata
    // survives.
    any_delegate!(&any, s => {
        t.name = s.name.clone();
        t.props = s.props.clone();
        t.anonymous = s.anonymous;
        t.is_trie = s.is_trie;
    });
    Ok(t)
}

impl FromAnyTransducer for StdVectorFst {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::Tropical(t) => Ok(t),
            other @ AnyTransducer::OlW(_) | other @ AnyTransducer::OlU(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
            other @ AnyTransducer::Thfst(_) => any_into_backend_via_basic(other),
        }
    }
}

impl FromAnyTransducer for Transducer<WeightedTables> {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::OlW(t) => Ok(t),
            // THFST <-> OLW is an O(1) table MOVE, not a round-trip through the
            // basic transducer: recover the inner engine and rewrap the facade
            // metadata unchanged.
            // [spec:hfst:def:thfst-backend.olw-moves]
            // [spec:hfst:sem:thfst-backend.olw-moves]
            AnyTransducer::Thfst(t) => Ok(rewrap_facade(t, |b| b.into_ol())),
            other @ AnyTransducer::Tropical(_) | other @ AnyTransducer::OlU(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
        }
    }
}

impl FromAnyTransducer for Transducer<UnweightedTables> {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::OlU(t) => Ok(t),
            // Any other source would need 'from_basic' into unweighted-shaped
            // tables, which the interim invariant of
            // [dec:hfst:monomorphic-backends] rules out (conversions always
            // build weighted-shaped tables); 'from_basic' reports that.
            other @ AnyTransducer::Tropical(_) | other @ AnyTransducer::OlW(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
            other @ AnyTransducer::Thfst(_) => any_into_backend_via_basic(other),
        }
    }
}

#[cfg(feature = "foma")]
impl FromAnyTransducer for crate::backend_foma::FomaTransducer {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::Foma(t) => Ok(t),
            other @ AnyTransducer::Tropical(_)
            | other @ AnyTransducer::OlW(_)
            | other @ AnyTransducer::OlU(_)
            | other @ AnyTransducer::Thfst(_) => any_into_backend_via_basic(other),
        }
    }
}

impl FromAnyTransducer for crate::backend_thfst::ThfstTransducer {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::Thfst(t) => Ok(t),
            // THFST <-> OLW is an O(1) table MOVE: transfer the inner engine
            // and rewrap the facade metadata unchanged.
            // [spec:hfst:sem:thfst-backend.olw-moves]
            AnyTransducer::OlW(t) => Ok(rewrap_facade(t, crate::backend_thfst::ThfstTransducer)),
            other @ AnyTransducer::Tropical(_) | other @ AnyTransducer::OlU(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
        }
    }
}
