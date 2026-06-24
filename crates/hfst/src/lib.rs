//! `hfst` — the public facade: `HfstTransducer` and the backend-dispatching API.
//!
//! Wave 2 targets: `HfstTransducer`, `HfstInputStream`/`HfstOutputStream`, the
//! Tropical/Log weight wrappers over `hfst-openfst`, `compose_intersect/*`,
//! `HfstRules`/`HfstXeroxRules`/`HfstApply`, and the print/extract surface.
//!
//! Backend union variants in scope: Tropical/Log (hfst-openfst), OL (hfst-ol),
//! and the `HfstBasicTransducer` interchange type (hfst-core). SFST/foma/xfsm
//! are out of scope for this phase.
