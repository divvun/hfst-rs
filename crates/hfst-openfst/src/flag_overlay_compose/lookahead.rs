//! Label-reachability pruning for product-heavy overlay composition.

use std::borrow::Borrow;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use rustfst::algorithms::compose::LabelReachableData;
use rustfst::algorithms::compose::lookahead_matchers::{LookAheadMatcherData, LookaheadMatcher};
use rustfst::fst_traits::{ExpandedFst, MutableFst};

use super::*;

#[derive(Clone, Debug, Default)]
pub(super) struct LookaheadPeerIndex {
    words_per_state: usize,
    reachable_labels: Box<[u64]>,
    matcher_reachable_labels: Option<Box<[u64]>>,
    unconditionally_reachable: Box<[bool]>,
    compact_to_mapped_label: Box<[Label]>,
}

const LOOKAHEAD_PAIR_CACHE_SIZE: usize = 1 << 20;
const LOOKAHEAD_INDEX_MAX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub(super) struct LookaheadPairCache {
    reachable: Box<[AtomicU64]>,
    unreachable: Box<[AtomicU64]>,
}

impl LookaheadPairCache {
    fn new() -> Self {
        Self {
            reachable: (0..LOOKAHEAD_PAIR_CACHE_SIZE)
                .map(|_| AtomicU64::new(u64::MAX))
                .collect(),
            unreachable: (0..LOOKAHEAD_PAIR_CACHE_SIZE)
                .map(|_| AtomicU64::new(u64::MAX))
                .collect(),
        }
    }

    fn slot(key: u64) -> usize {
        let mut hash = key.wrapping_add(0x9e37_79b9_7f4a_7c15);
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((hash ^ (hash >> 31)) as usize) & (LOOKAHEAD_PAIR_CACHE_SIZE - 1)
    }

    fn get(&self, key: u64) -> Option<bool> {
        let slot = Self::slot(key);
        if self.reachable[slot].load(Ordering::Relaxed) == key {
            return Some(true);
        }
        (self.unreachable[slot].load(Ordering::Relaxed) == key).then_some(false)
    }

    fn insert(&self, key: u64, reachable: bool) {
        let slot = Self::slot(key);
        if reachable {
            self.reachable[slot].store(key, Ordering::Relaxed);
        } else {
            self.unreachable[slot].store(key, Ordering::Relaxed);
        }
    }
}

impl OverlayMatcher {
    pub(super) fn with_lookahead(mut self) -> Result<Self> {
        let reach_input = match self.match_type {
            MatchType::MatchInput => true,
            MatchType::MatchOutput => false,
            MatchType::MatchBoth | MatchType::MatchNone | MatchType::MatchUnknown => {
                bail!("flag-overlay label lookahead requires an input or output matcher")
            }
        };

        // A flag physically present only on this operand is matched by a
        // virtual loop at every state of the other operand. It therefore must
        // behave like epsilon in the reachability index: the useful question
        // is which ordinary label comes after any run of such flags.
        let source = self.base.fst().as_ref();
        let mut reachability_fst = StdVectorFst::new();
        reachability_fst.add_states(source.num_states());
        if let Some(start) = source.start() {
            reachability_fst.set_start(start)?;
        }
        for state in 0..source.num_states() as StateId {
            if let Some(weight) = source.final_weight(state)? {
                reachability_fst.set_final(state, weight)?;
            }
            for transition in source.get_trs(state)?.trs() {
                let mut transition = transition.clone();
                let label = if reach_input {
                    &mut transition.ilabel
                } else {
                    &mut transition.olabel
                };
                if self.listed_labels.binary_search(label).is_ok() {
                    *label = EPS_LABEL;
                }
                reachability_fst.add_tr(state, transition)?;
            }
        }
        let lookahead = LabelReachable::new(&reachability_fst, reach_input)?;
        self.set_lookahead(lookahead);
        Ok(self)
    }

    fn set_lookahead(&mut self, lookahead: LabelReachable) {
        let mut label_map: Vec<_> = lookahead
            .data()
            .label2index()
            .iter()
            .map(|(label, mapped)| (*label, *mapped))
            .collect();
        label_map.sort_unstable_by_key(|(label, _)| *label);
        let max_label = label_map.last().map_or(0, |(label, _)| *label as usize);
        if max_label <= 1_000_000 {
            let mut dense = vec![NO_LABEL; max_label.saturating_add(1)];
            for (label, mapped) in label_map {
                dense[label as usize] = mapped;
            }
            self.lookahead_dense_label_map = Some(Arc::from(dense));
            self.lookahead_label_map = Arc::from([]);
        } else {
            self.lookahead_dense_label_map = None;
            self.lookahead_label_map = Arc::from(label_map);
        }
        self.lookahead = Some(lookahead);
    }

    fn mapped_lookahead_label(&self, label: Label) -> Option<Label> {
        if let Some(dense) = &self.lookahead_dense_label_map {
            let mapped = *dense.get(label as usize)?;
            return (mapped != NO_LABEL).then_some(mapped);
        }
        self.lookahead_label_map
            .binary_search_by_key(&label, |(original, _)| *original)
            .ok()
            .map(|index| self.lookahead_label_map[index].1)
    }

    pub(super) fn index_lookahead_peer(
        &mut self,
        fst: &StdVectorFst,
        memory_cap_bytes: Option<u64>,
    ) -> Result<u64> {
        let Some(lookahead) = &self.lookahead else {
            return Ok(0);
        };
        let mut compact_to_mapped_label: Vec<_> =
            lookahead.data().label2index().values().copied().collect();
        compact_to_mapped_label.sort_unstable();
        compact_to_mapped_label.dedup();
        let state_count = fst.num_states();
        let words_per_state = compact_to_mapped_label.len().div_ceil(64);
        let total_words = state_count.checked_mul(words_per_state).ok_or_else(|| {
            anyhow::anyhow!("lookahead peer label index exceeds addressable memory")
        })?;
        let peer_index_bytes = total_words
            .checked_mul(std::mem::size_of::<u64>())
            .and_then(|bytes| {
                state_count
                    .checked_mul(std::mem::size_of::<bool>())
                    .and_then(|state_bytes| bytes.checked_add(state_bytes))
            })
            .and_then(|bytes| {
                compact_to_mapped_label
                    .len()
                    .checked_mul(std::mem::size_of::<Label>())
                    .and_then(|label_bytes| bytes.checked_add(label_bytes))
            })
            .ok_or_else(|| anyhow::anyhow!("lookahead peer label index size overflow"))?;
        let allowed_bytes = memory_cap_bytes
            .unwrap_or(LOOKAHEAD_INDEX_MAX_BYTES)
            .min(LOOKAHEAD_INDEX_MAX_BYTES);
        if u64::try_from(peer_index_bytes).unwrap_or(u64::MAX) > allowed_bytes {
            // Lookahead is only an optimization. Falling back to the ordinary
            // per-state check is conservative and keeps the configured pair
            // store budget available for actual product states.
            return Ok(0);
        }
        let max_mapped_label = compact_to_mapped_label.last().copied().unwrap_or(0) as usize;
        let mut mapped_to_compact = vec![usize::MAX; max_mapped_label.saturating_add(1)];
        for (compact, mapped) in compact_to_mapped_label.iter().copied().enumerate() {
            mapped_to_compact[mapped as usize] = compact;
        }
        let mut reachable_labels = vec![0_u64; total_words];
        let mut unconditionally_reachable = vec![false; state_count];
        let mut epsilon_predecessors = vec![Vec::new(); state_count];

        for state in 0..fst.num_states() as StateId {
            for transition in fst.get_trs(state)?.trs() {
                let label = if self.match_type == MatchType::MatchInput {
                    transition.olabel
                } else {
                    transition.ilabel
                };
                if label == EPS_LABEL {
                    epsilon_predecessors[transition.nextstate as usize].push(state as usize);
                    continue;
                }
                if self.loop_labels.binary_search(&label).is_ok() {
                    unconditionally_reachable[state as usize] = true;
                    continue;
                }
                let Some(mapped) = self.mapped_lookahead_label(label) else {
                    continue;
                };
                let compact = mapped_to_compact[mapped as usize];
                debug_assert_ne!(compact, usize::MAX);
                let offset = state as usize * words_per_state + compact / 64;
                reachable_labels[offset] |= 1_u64 << (compact % 64);
            }
            if fst
                .final_weight(state)?
                .is_some_and(|weight| !weight.is_zero())
            {
                let final_label = lookahead.data().final_label();
                if final_label != NO_LABEL {
                    let compact = mapped_to_compact[final_label as usize];
                    debug_assert_ne!(compact, usize::MAX);
                    let offset = state as usize * words_per_state + compact / 64;
                    reachable_labels[offset] |= 1_u64 << (compact % 64);
                }
            }
        }

        // Propagate each state's possible next labels backwards over the peer
        // operand's epsilon graph. This is the exact one-symbol future needed
        // by composition: a peer epsilon no longer makes every pair look
        // viable merely because some incompatible label follows it.
        let mut queue: VecDeque<_> = (0..state_count).collect();
        let mut queued = vec![true; state_count];
        while let Some(state) = queue.pop_front() {
            queued[state] = false;
            for predecessor in epsilon_predecessors[state].iter().copied() {
                let mut changed = false;
                if unconditionally_reachable[state] && !unconditionally_reachable[predecessor] {
                    unconditionally_reachable[predecessor] = true;
                    changed = true;
                }
                let source = state * words_per_state;
                let target = predecessor * words_per_state;
                for word in 0..words_per_state {
                    let combined =
                        reachable_labels[target + word] | reachable_labels[source + word];
                    if combined != reachable_labels[target + word] {
                        reachable_labels[target + word] = combined;
                        changed = true;
                    }
                }
                if changed && !queued[predecessor] {
                    queued[predecessor] = true;
                    queue.push_back(predecessor);
                }
            }
        }

        let mut used_bytes = peer_index_bytes;
        let matcher_state_count = self.base.fst().num_states();
        let matcher_total_words = matcher_state_count
            .checked_mul(words_per_state)
            .ok_or_else(|| anyhow::anyhow!("lookahead matcher label index size overflow"))?;
        let matcher_bytes = matcher_total_words
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or_else(|| anyhow::anyhow!("lookahead matcher label index size overflow"))?;
        let matcher_reachable_labels = if u64::try_from(used_bytes.saturating_add(matcher_bytes))
            .unwrap_or(u64::MAX)
            <= allowed_bytes
        {
            let mut labels = vec![0_u64; matcher_total_words];
            for state in 0..matcher_state_count as StateId {
                let intervals = lookahead.data().interval_set(state)?;
                let state_offset = state as usize * words_per_state;
                for interval in intervals.iter() {
                    let mut compact = compact_to_mapped_label
                        .partition_point(|mapped| (*mapped as usize) < interval.begin);
                    while compact < compact_to_mapped_label.len()
                        && (compact_to_mapped_label[compact] as usize) < interval.end
                    {
                        labels[state_offset + compact / 64] |= 1_u64 << (compact % 64);
                        compact += 1;
                    }
                }
            }
            used_bytes += matcher_bytes;
            Some(labels.into_boxed_slice())
        } else {
            None
        };

        let cache_bytes = LOOKAHEAD_PAIR_CACHE_SIZE
            .checked_mul(2 * std::mem::size_of::<AtomicU64>())
            .ok_or_else(|| anyhow::anyhow!("lookahead pair cache size overflow"))?;
        let lookahead_pair_cache = if u64::try_from(used_bytes.saturating_add(cache_bytes))
            .unwrap_or(u64::MAX)
            <= allowed_bytes
        {
            used_bytes += cache_bytes;
            Some(Arc::new(LookaheadPairCache::new()))
        } else {
            None
        };

        self.lookahead_peer_index = Some(Arc::new(LookaheadPeerIndex {
            words_per_state,
            reachable_labels: reachable_labels.into_boxed_slice(),
            matcher_reachable_labels,
            unconditionally_reachable: unconditionally_reachable.into_boxed_slice(),
            compact_to_mapped_label: compact_to_mapped_label.into_boxed_slice(),
        }));
        self.lookahead_pair_cache = lookahead_pair_cache;
        Ok(u64::try_from(used_bytes).unwrap_or(u64::MAX))
    }

    fn indexed_peer_is_reachable(
        &self,
        matcher_state: StateId,
        peer_state: StateId,
    ) -> Result<Option<bool>> {
        let (Some(lookahead), Some(peer_index)) = (&self.lookahead, &self.lookahead_peer_index)
        else {
            return Ok(None);
        };
        let key = (u64::from(matcher_state) << 32) | u64::from(peer_state);
        debug_assert_ne!(key, u64::MAX, "NO_STATE_ID is not a product state");
        if let Some(reachable) = self
            .lookahead_pair_cache
            .as_ref()
            .and_then(|cache| cache.get(key))
        {
            return Ok(Some(reachable));
        }
        let peer_state = peer_state as usize;
        let unconditional = peer_index
            .unconditionally_reachable
            .get(peer_state)
            .ok_or_else(|| anyhow::anyhow!("missing lookahead peer state {peer_state}"))?;
        if *unconditional {
            if let Some(cache) = &self.lookahead_pair_cache {
                cache.insert(key, true);
            }
            return Ok(Some(true));
        }

        let intervals = lookahead.data().interval_set(matcher_state)?;
        let start = peer_state
            .checked_mul(peer_index.words_per_state)
            .ok_or_else(|| anyhow::anyhow!("lookahead peer-state offset overflow"))?;
        let words = peer_index
            .reachable_labels
            .get(start..start + peer_index.words_per_state)
            .ok_or_else(|| {
                anyhow::anyhow!("missing lookahead peer labels for state {peer_state}")
            })?;
        if let Some(matcher_labels) = &peer_index.matcher_reachable_labels {
            let matcher_start = (matcher_state as usize)
                .checked_mul(peer_index.words_per_state)
                .ok_or_else(|| anyhow::anyhow!("lookahead matcher-state offset overflow"))?;
            let matcher_words = matcher_labels
                .get(matcher_start..matcher_start + peer_index.words_per_state)
                .ok_or_else(|| {
                    anyhow::anyhow!("missing lookahead matcher labels for state {matcher_state}")
                })?;
            let reachable = words
                .iter()
                .zip(matcher_words)
                .any(|(peer, matcher)| peer & matcher != 0);
            if let Some(cache) = &self.lookahead_pair_cache {
                cache.insert(key, reachable);
            }
            return Ok(Some(reachable));
        }

        for (word_index, word) in words.iter().copied().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                let bit = remaining.trailing_zeros() as usize;
                let compact = word_index * 64 + bit;
                let mapped = peer_index.compact_to_mapped_label[compact] as usize;
                if intervals.member(mapped) {
                    if let Some(cache) = &self.lookahead_pair_cache {
                        cache.insert(key, true);
                    }
                    return Ok(Some(true));
                }
                remaining &= remaining - 1;
            }
        }
        if let Some(cache) = &self.lookahead_pair_cache {
            cache.insert(key, false);
        }
        Ok(Some(false))
    }
}

impl LookaheadMatcher<TropicalWeight, StdVectorFst, FstHandle> for OverlayMatcher {
    type MatcherData = LabelReachableData;

    fn data(&self) -> Option<&Arc<Self::MatcherData>> {
        self.lookahead.as_ref().map(LabelReachable::data)
    }

    fn new_with_data(
        fst: FstHandle,
        match_type: MatchType,
        data: Option<Arc<Self::MatcherData>>,
    ) -> Result<Self> {
        let mut matcher = Self::new(Arc::clone(&fst), match_type)?;
        if matches!(match_type, MatchType::MatchInput | MatchType::MatchOutput) {
            let reach_input = match_type == MatchType::MatchInput;
            let data = match data {
                Some(data) => data,
                None => Arc::new(LabelReachable::compute_data(fst.as_ref(), reach_input)?),
            };
            matcher.set_lookahead(LabelReachable::new_from_data(data));
        }
        Ok(matcher)
    }

    fn create_data<F: Fst<TropicalWeight>, B: Borrow<F>>(
        fst: B,
        match_type: MatchType,
    ) -> Result<Option<Self::MatcherData>> {
        if matches!(match_type, MatchType::MatchInput | MatchType::MatchOutput) {
            Ok(Some(LabelReachable::compute_data(
                fst.borrow(),
                match_type == MatchType::MatchInput,
            )?))
        } else {
            Ok(None)
        }
    }

    fn init_lookahead_fst<F: Fst<TropicalWeight>, B: Borrow<F> + Clone>(
        &mut self,
        fst: &B,
    ) -> Result<()> {
        if let Some(lookahead) = &mut self.lookahead {
            lookahead.reach_init(fst.borrow(), self.match_type == MatchType::MatchOutput)?;
        }
        Ok(())
    }

    fn lookahead_fst<F: Fst<TropicalWeight>, B: Borrow<F>>(
        &self,
        matcher_state: StateId,
        fst: &B,
        fst_state: StateId,
    ) -> Result<Option<LookAheadMatcherData<TropicalWeight>>> {
        let Some(lookahead) = &self.lookahead else {
            return Ok(Some(LookAheadMatcherData::default()));
        };

        if let Some(reachable) = self.indexed_peer_is_reachable(matcher_state, fst_state)? {
            return Ok(reachable.then(LookAheadMatcherData::default));
        }

        let fst = fst.borrow();
        let transitions = fst.get_trs(fst_state)?;
        for transition in transitions.trs() {
            let label = if self.match_type == MatchType::MatchInput {
                transition.olabel
            } else {
                transition.ilabel
            };
            if label == EPS_LABEL || self.loop_labels.binary_search(&label).is_ok() {
                return Ok(Some(LookAheadMatcherData::default()));
            }
            if let Some(mapped) = self.mapped_lookahead_label(label)
                && lookahead.reach_label(matcher_state, mapped)?
            {
                return Ok(Some(LookAheadMatcherData::default()));
            }
        }

        if fst
            .final_weight(fst_state)?
            .is_some_and(|weight| !weight.is_zero())
            && lookahead.reach_final(matcher_state)?
        {
            return Ok(Some(LookAheadMatcherData::default()));
        }
        Ok(None)
    }

    fn lookahead_label(&self, state: StateId, label: Label) -> Result<bool> {
        if label == EPS_LABEL || self.loop_labels.binary_search(&label).is_ok() {
            return Ok(true);
        }
        let Some(lookahead) = &self.lookahead else {
            return Ok(true);
        };
        let Some(mapped) = self.mapped_lookahead_label(label) else {
            return Ok(false);
        };
        lookahead.reach_label(state, mapped)
    }

    fn lookahead_prefix(
        &self,
        _transition: &mut Tr<TropicalWeight>,
        _data: &LookAheadMatcherData<TropicalWeight>,
    ) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub(super) struct OverlayLookAheadComposeFilterBuilder {
    pub(super) base: OverlayComposeFilterBuilder,
}

impl
    ComposeFilterBuilder<
        TropicalWeight,
        StdVectorFst,
        StdVectorFst,
        FstHandle,
        FstHandle,
        OverlayMatcher,
        OverlayMatcher,
    > for OverlayLookAheadComposeFilterBuilder
{
    type IM1 = OverlayMatcher;
    type IM2 = OverlayMatcher;
    type CF = OverlayLookAheadComposeFilter;

    fn new(
        fst1: FstHandle,
        fst2: FstHandle,
        matcher1: Option<OverlayMatcher>,
        matcher2: Option<OverlayMatcher>,
    ) -> Result<Self> {
        Ok(Self {
            base: OverlayComposeFilterBuilder::new(fst1, fst2, matcher1, matcher2)?,
        })
    }

    fn build(&self) -> Result<Self::CF> {
        Ok(OverlayLookAheadComposeFilter {
            base: self.base.build()?,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct OverlayLookAheadComposeFilter {
    base: OverlayComposeFilter,
}

impl
    ComposeFilter<
        TropicalWeight,
        StdVectorFst,
        StdVectorFst,
        FstHandle,
        FstHandle,
        OverlayMatcher,
        OverlayMatcher,
    > for OverlayLookAheadComposeFilter
{
    type FS = OverlayFilterState;

    fn start(&self) -> Self::FS {
        self.base.start()
    }

    fn set_state(&mut self, s1: StateId, s2: StateId, state: &Self::FS) -> Result<()> {
        self.base.set_state(s1, s2, state)
    }

    fn filter_tr(
        &mut self,
        arc1: &mut Tr<TropicalWeight>,
        arc2: &mut Tr<TropicalWeight>,
    ) -> Result<Self::FS> {
        let reachable = self
            .base
            .matcher1
            .lookahead_fst::<StdVectorFst, FstHandle>(
                arc1.nextstate,
                self.base.matcher2.fst(),
                arc2.nextstate,
            )?;
        if reachable.is_none() {
            return Ok(Self::FS::new_no_state());
        }
        self.base.filter_tr(arc1, arc2)
    }

    fn filter_final(
        &self,
        weight1: &mut TropicalWeight,
        weight2: &mut TropicalWeight,
    ) -> Result<()> {
        self.base.filter_final(weight1, weight2)
    }

    fn matcher1(&self) -> &OverlayMatcher {
        self.base.matcher1()
    }

    fn matcher2(&self) -> &OverlayMatcher {
        self.base.matcher2()
    }

    fn matcher1_shared(&self) -> &Arc<OverlayMatcher> {
        self.base.matcher1_shared()
    }

    fn matcher2_shared(&self) -> &Arc<OverlayMatcher> {
        self.base.matcher2_shared()
    }

    fn properties(&self, properties: FstProperties) -> FstProperties {
        self.base.properties(properties)
    }
}

type InnerLookAheadOverlayComposeFst = ComposeFst<
    TropicalWeight,
    StdVectorFst,
    StdVectorFst,
    FstHandle,
    FstHandle,
    OverlayMatcher,
    OverlayMatcher,
    OverlayLookAheadComposeFilterBuilder,
    NullCache<TropicalWeight>,
>;

/// A one-shot overlay composition that rejects product pairs whose future
/// labels cannot meet before it interns or materializes those pairs.
///
/// This is intentionally a separate constructor from [`FlagOverlayComposeFst`]:
/// building the right operand's label-reachability index has a fixed whole-FST
/// cost, so ordinary small compositions retain the cheaper sequence-filter
/// path while compose-intersect can opt into aggressive product pruning.
#[derive(Debug)]
pub struct FlagOverlayLookAheadComposeFst {
    pub(super) inner: InnerLookAheadOverlayComposeFst,
}

impl FlagOverlayLookAheadComposeFst {
    pub fn new_with_state_store(
        fst1: FstHandle,
        fst2: FstHandle,
        overlay: FlagOverlay,
        mut state_store: Option<ComposeStateStoreConfig>,
    ) -> Result<Self> {
        let mut matcher1 = OverlayMatcher::with_overlay(
            Arc::clone(&fst1),
            MatchType::MatchOutput,
            Arc::clone(&overlay.left_self_loops),
            Arc::clone(&overlay.right_self_loops),
            overlay.flags_as_epsilon,
        )?
        .with_lookahead()?;
        matcher1.init_lookahead_fst::<StdVectorFst, FstHandle>(&fst2)?;
        let index_memory_bytes = matcher1.index_lookahead_peer(
            fst2.as_ref(),
            state_store.as_ref().map(|store| store.memory_cap_bytes),
        )?;
        if let Some(store) = &mut state_store {
            store.memory_cap_bytes = store.memory_cap_bytes.saturating_sub(index_memory_bytes);
        }
        let matcher2 = OverlayMatcher::with_overlay(
            Arc::clone(&fst2),
            MatchType::MatchInput,
            Arc::clone(&overlay.right_self_loops),
            Arc::clone(&overlay.left_self_loops),
            overlay.flags_as_epsilon,
        )?;
        let filter_builder = OverlayLookAheadComposeFilterBuilder {
            base: OverlayComposeFilterBuilder::with_overlay(
                Arc::clone(&fst1),
                Arc::clone(&fst2),
                Some(matcher1.clone()),
                Some(matcher2.clone()),
                overlay,
            )?,
        };
        let options: ComposeFstOpOptions<
            OverlayMatcher,
            OverlayMatcher,
            OverlayLookAheadComposeFilterBuilder,
            OverlayOpState,
        > = ComposeFstOpOptions::new(
            Some(matcher1),
            Some(matcher2),
            Some(filter_builder),
            state_store.map(OverlayOpState::new_spillable).transpose()?,
        );
        let inner = InnerLookAheadOverlayComposeFst::new_with_options_and_cache(
            fst1,
            fst2,
            options,
            NullCache::default(),
        )?;
        Ok(Self { inner })
    }

    pub fn as_fst(&self) -> &impl Fst<TropicalWeight> {
        &self.inner
    }
}

/// Storage-aware label-lookahead composition for product-heavy workloads.
pub fn compose_lookahead_with_store(
    fst1: FstHandle,
    fst2: FstHandle,
    overlay: FlagOverlay,
    state_store: Option<ComposeStateStoreConfig>,
) -> Result<FlagOverlayLookAheadComposeFst> {
    FlagOverlayLookAheadComposeFst::new_with_state_store(fst1, fst2, overlay, state_store)
}
