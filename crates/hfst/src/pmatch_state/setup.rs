//! Starting a run over a loaded archive, and the knobs that configure it.

use super::*;

impl PmatchContainer {
    // PmatchContainer(void)
    // Not used, but apparently needed by swig to construct these
    pub fn new() -> PmatchContainer {
        PmatchContainer::from_core(Arc::new(PmatchCore::new()))
    }

    /// A fresh run over an already-loaded archive. The core is shared, not
    /// copied, so this is the constructor a second thread calls.
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn from_core(core: Arc<PmatchCore>) -> PmatchContainer {
        PmatchContainer {
            input: SymbolNumberVector::new(),
            entry_stack: Vec::new(),
            rtn_stacks: RtnCallStacks::new(),
            tape: DoubleTape::new(),
            best_result: DoubleTape::new(),
            result: DoubleTape::new(),
            locations: LocationVectorVector::new(),
            tape_locations: WeightedDoubleTapeVector::new(),
            captures: Vec::new(),
            best_captures: Vec::new(),
            old_captures: Vec::new(),
            global_flag_state: core.flag_state_proto.clone(),
            verbose: false,
            props: core.props,
            line_number: 0,
            pattern_counts: BTreeMap::new(),
            counters: Vec::new(),
            profile_mode: false,
            single_codepoint_tokenization: false,
            recursion_depth_left: core.props.max_recursion as u32,
            max_time: 0.0,
            start_clock: None,
            call_counter: 0,
            limit_reached: false,
            max_weight: crate::transducer::INFINITE_WEIGHT,
            running_weight: 0.0,
            weight_limit: crate::transducer::INFINITE_WEIGHT,
            stack_depth: 0,
            best_input_pos: 0,
            best_weight: 0.0,
            epsilon_path: Vec::new(),
            unicode_cache: Vec::new(),
            overlay: None,
            core,
        }
    }

    /// A handle on the loaded archive, for starting further runs over it.
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn core(&self) -> Arc<PmatchCore> {
        Arc::clone(&self.core)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    pub fn new_from_stream(is: &mut dyn std::io::BufRead) -> crate::error::Result<PmatchContainer> {
        Ok(PmatchContainer::from_core(Arc::new(
            PmatchCore::from_stream(is)?,
        )))
    }

    // PmatchContainer(Transducer *t)
    pub fn new_from_transducer(
        toplevel: crate::transducer::Transducer,
    ) -> crate::error::Result<PmatchContainer> {
        Ok(PmatchContainer::from_core(Arc::new(
            PmatchCore::from_transducer(toplevel)?,
        )))
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // explicit PmatchContainer(std::vector<hfst::HfstTransducer>)
    pub fn new_from_hfst_transducers(
        transducers: Vec<crate::hfst_transducer::HfstTransducer<crate::transducer::Transducer>>,
    ) -> crate::error::Result<PmatchContainer> {
        Ok(PmatchContainer::from_core(Arc::new(
            PmatchCore::from_hfst_transducers(transducers)?,
        )))
    }

    // void set_properties(void)
    pub fn set_properties(&mut self) {
        self.props = PmatchProperties::new();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    pub fn set_properties_map(&mut self, properties: &BTreeMap<String, String>) {
        self.props.set_from_map(properties);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.parse-hfst3-header-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.parse-hfst3-header-fn]
    pub fn parse_hfst3_header(
        f: &mut dyn std::io::BufRead,
    ) -> crate::error::Result<BTreeMap<String, String>> {
        // 'HFST' plus the C-string NUL.
        const MAGIC: &[u8] = b"HFST\0";
        // The C++ read the magic byte by byte and, on a mismatch, put every byte
        // it had taken back, so a stream that turns out not to start a header is
        // left exactly where the caller handed it over. Matching against the
        // buffer and consuming only on success keeps that property; a reader
        // that cannot serve five bytes from one fill has no header to offer.
        let matched = match f.fill_buf() {
            Ok(buf) => buf.starts_with(MAGIC),
            Err(_) => false,
        };
        if !matched {
            crate::bail!(TransducerHeader);
        }
        f.consume(MAGIC.len());

        let mut len_bytes = [0u8; 2];
        f.read_exact(&mut len_bytes)
            .map_err(|_| crate::err!(TransducerHeader))?;
        let remaining_header_len = u16::from_ne_bytes(len_bytes) as usize;
        let mut nul = [0u8; 1];
        f.read_exact(&mut nul)
            .map_err(|_| crate::err!(TransducerHeader))?;
        if nul[0] != 0 {
            crate::bail!(TransducerHeader);
        }
        let mut headervalue = vec![0u8; remaining_header_len];
        f.read_exact(&mut headervalue)
            .map_err(|_| crate::err!(TransducerHeader))?;
        if remaining_header_len == 0 || headervalue[remaining_header_len - 1] != 0 {
            crate::bail!(TransducerHeader);
        }

        let mut properties: BTreeMap<String, String> = BTreeMap::new();
        let cstrlen = |s: &[u8]| -> usize { s.iter().position(|&b| b == 0).unwrap_or(s.len()) };
        let mut i = 0usize;
        while i < remaining_header_len {
            let length = cstrlen(&headervalue[i..]);
            let property = String::from_utf8_lossy(&headervalue[i..i + length]).into_owned();
            i += length + 1;
            let length = cstrlen(&headervalue[i..]);
            let value = String::from_utf8_lossy(&headervalue[i..i + length]).into_owned();
            properties.insert(property, value);
            i += length + 1;
        }
        Ok(properties)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-verbose-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-verbose-fn]
    pub fn set_verbose(&mut self, b: bool) {
        self.verbose = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-locate-mode-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-locate-mode-fn]
    pub fn set_locate_mode(&mut self, b: bool) {
        self.props.locate_mode = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
    pub fn set_extract_patterns(&mut self, b: bool) {
        self.props.extract_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
    pub fn set_single_codepoint_tokenization(&mut self, b: bool) {
        self.single_codepoint_tokenization = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
    pub fn set_count_patterns(&mut self, b: bool) {
        self.props.count_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
    pub fn set_delete_patterns(&mut self, b: bool) {
        self.props.delete_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
    pub fn set_mark_patterns(&mut self, b: bool) {
        self.props.mark_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
    pub fn set_max_recursion(&mut self, max: usize) {
        self.props.max_recursion = max;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
    pub fn set_max_context(&mut self, max: usize) {
        self.props.max_context_length = max;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
    pub fn is_in_locate_mode(&self) -> bool {
        self.props.locate_mode
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-profile-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-profile-fn]
    pub fn set_profile(&mut self, b: bool) {
        self.profile_mode = b;
    }

    pub fn has_multichar_input_symbols(&self) -> bool {
        self.core.has_multichar_input_symbols()
    }
}
