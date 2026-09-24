//! Exhaustive and random path extraction.

use super::*;
use crate::hfst_data_types::HfstTwoLevelPath;
use crate::hfst_flag_diacritics::FdState;
use crate::hfst_lookup_flag_diacritics::FlagDiacriticTable;
use crate::hfst_symbol_defs::symbols::{remove_flags_two_level_path, to_string_vector_from_path};

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.label-pair]
pub type LabelPair = (i32, i32);
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.label-pair-vector]
pub type LabelPairVector = Vec<LabelPair>;

// ============================================================================
// File-static free helpers (C++ 'static' functions in
// 'namespace hfst::implementations').  Kept module-private, like the C++.
// ============================================================================

/* The recursive path-extraction worker.  Note the faithful C++ quirk that
`all_visitations` / `path_visitations` are passed *by value* (each recursive
call gets its own copy), while `spv` and `fd_state_stack` are shared
(passed by reference / pointer). */
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.extract-paths-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.extract-paths-fn]
#[allow(clippy::too_many_arguments)]
fn extract_paths(
    t: &StdVectorFst,
    s: StateId,
    mut all_visitations: BTreeMap<StateId, u16>,
    mut path_visitations: BTreeMap<StateId, u16>,
    weight_sum: f32,
    callback: &mut dyn ExtractStringsCb,
    cycles: i32,
    mut fd_state_stack: Option<&mut Vec<FdState<i64>>>,
    filter_fd: bool,
    spv: &mut StringPairVector,
) -> bool {
    if cycles >= 0 && (*path_visitations.entry(s).or_insert(0) as i32) > cycles {
        return true;
    }
    *all_visitations.entry(s).or_insert(0) += 1;
    *path_visitations.entry(s).or_insert(0) += 1;

    if !spv.is_empty() {
        let is_final = t.is_final(s).expect("s is a valid state of this fst");
        let fw = if is_final {
            *t.final_weight(s)
                .expect("s is a valid state of this fst")
                .expect("state confirmed final via is_final")
                .value()
        } else {
            0.0
        };
        let mut path = HfstTwoLevelPath {
            first: weight_sum + fw,
            second: spv.clone(),
        };
        let ret = callback.operator_call(&mut path, is_final);
        if !ret.continueSearch || !ret.continuePath {
            *path_visitations.entry(s).or_insert(0) -= 1;
            return ret.continueSearch;
        }
    }

    // sort arcs by number of visitations (stable insertion sort, ascending)
    let mut arcs: Vec<StdTransition> = Vec::new();
    for a in t.get_trs(s).expect("s is a valid state of this fst").trs() {
        let mut i = 0usize;
        while i < arcs.len() {
            let av_a = *all_visitations.get(&a.nextstate).unwrap_or(&0);
            let av_i = *all_visitations.get(&arcs[i].nextstate).unwrap_or(&0);
            if av_a < av_i {
                break;
            }
            i += 1;
        }
        arcs.insert(i, a.clone());
    }

    let mut res = true;
    let mut idx = 0usize;
    while idx < arcs.len() && res {
        let arc = arcs[idx].clone();
        let mut added_fd_state = false;

        if let Some(stack) = fd_state_stack.as_deref_mut()
            && stack
                .last()
                .expect("fd state stack is non-empty")
                .get_table()
                .get_operation(arc.ilabel as i64)
                .is_some()
        {
            let top = stack.last().expect("fd state stack is non-empty").clone();
            stack.push(top);
            if stack
                .last_mut()
                .expect("fd state stack is non-empty")
                .apply_operation_symbol(arc.ilabel as i64)
            {
                added_fd_state = true;
            } else {
                stack.pop();
                idx += 1;
                continue; // don't follow the transition
            }
        }

        /* Handle spv here. Special symbols (flags, epsilons) are always
        inserted. */
        let mut istring = crate::hfst_data_types::Symbol::default();
        let mut ostring = crate::hfst_data_types::Symbol::default();

        if !filter_fd
            || fd_state_stack
                .as_deref()
                .expect("fd_state_stack present when filter_fd is set")
                .last()
                .expect("fd state stack is non-empty")
                .get_table()
                .get_operation(arc.ilabel as i64)
                .is_none()
        {
            istring = crate::hfst_data_types::Symbol::new(
                t.input_symbols()
                    .expect("transducer has an input symbol table")
                    .get_symbol(arc.ilabel)
                    .unwrap_or(""),
            );
        }

        if !filter_fd
            || fd_state_stack
                .as_deref()
                .expect("fd_state_stack present when filter_fd is set")
                .last()
                .expect("fd state stack is non-empty")
                .get_table()
                .get_operation(arc.olabel as i64)
                .is_none()
        {
            ostring = crate::hfst_data_types::Symbol::new(
                t.input_symbols()
                    .expect("transducer has an input symbol table")
                    .get_symbol(arc.olabel)
                    .unwrap_or(""),
            );
        }

        spv.push((istring, ostring));

        res = extract_paths(
            t,
            arc.nextstate,
            all_visitations.clone(),
            path_visitations.clone(),
            weight_sum + *arc.weight.value(),
            callback,
            cycles,
            fd_state_stack.as_deref_mut(),
            filter_fd,
            spv,
        );

        spv.pop();

        if added_fd_state {
            fd_state_stack
                .as_deref_mut()
                .expect("added_fd_state implies fd_state_stack is present")
                .pop();
        }
        idx += 1;
    }

    *path_visitations.entry(s).or_insert(0) -= 1;
    res
}

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.is-minimal-and-empty-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.is-minimal-and-empty-fn]
fn is_minimal_and_empty(t: &StdVectorFst) -> bool {
    let start_state = match t.start() {
        None => return true, // C++: start_state < 0
        Some(s) => s,
    };
    t.get_trs(start_state)
        .expect("start_state is a valid state of this fst")
        .trs()
        .is_empty()
}

// Tiny op-local xorshift PRNG replacing the C 'rand()'/'srand()' that the
// random-path extraction used (no global C RNG state). Created per
// extract_random_paths call and threaded down through random_path.
struct Rng {
    state: u64,
}
impl Rng {
    fn seeded(seed: u64) -> Self {
        Rng { state: seed | 1 }
    }
    fn next(&mut self) -> i32 {
        let mut z = self.state;
        z ^= z >> 12;
        z ^= z << 25;
        z ^= z >> 27;
        self.state = z;
        ((z.wrapping_mul(0x2545F4914F6CDD1D) >> 33) as i32) & i32::MAX
    }
}

/* Failure signals of the random-path extractor (the C++ threw C-strings
"transducer is empty" / "cannot extract random path" for these). */
enum RandomPathError {
    Empty,
    NoPath,
}

/* Per-state distance (in arcs) to the nearest final state, computed with a
backward BFS from the final states over the reversed edge set. `None` marks a
state from which no final state is reachable (not co-accessible). Computed
once per extract_random_paths call and used to steer the random walk toward
final states: the upstream C++ heuristic (hfst/hfst#444) followed arcs blind,
so on guesser/cyclic transducers whose accepting state sits behind a rare
deep suffix the walk almost never reached a final state and `-r` returned
nothing. The walk restricts its choices to co-accessible targets and, if the
short-path heuristic would otherwise abandon a walk that has not yet reached a
final state, descends this distance map to guarantee an accepting path. */
fn distance_to_final(t: &StdVectorFst) -> Vec<Option<u32>> {
    let n = t.num_states();
    let mut dist: Vec<Option<u32>> = vec![None; n];
    // Reverse edges: for each state, the predecessors that can step into it.
    let mut preds: Vec<Vec<StateId>> = vec![Vec::new(); n];
    let mut frontier: Vec<StateId> = Vec::new();
    for s in t.states_iter() {
        if t.is_final(s).expect("s is a valid state of this fst") {
            dist[s as usize] = Some(0);
            frontier.push(s);
        }
        for arc in t.get_trs(s).expect("s is a valid state of this fst").trs() {
            preds[arc.nextstate as usize].push(s);
        }
    }
    // Backward BFS from the final states over the reverse edges (level by
    // level, so the first time a state is reached is its shortest distance).
    let mut level = 0u32;
    while !frontier.is_empty() {
        level += 1;
        let mut next: Vec<StateId> = Vec::new();
        for s in frontier {
            for &p in &preds[s as usize] {
                if dist[p as usize].is_none() {
                    dist[p as usize] = Some(level);
                    next.push(p);
                }
            }
        }
        frontier = next;
    }
    dist
}

/* Extend `path` from `state` along a shortest route to a final state,
choosing randomly among the arcs that step strictly closer (per `dist`).
Guaranteed to terminate because every step decreases the distance-to-final and
`state` is co-accessible (`dist[state]` is Some). On arrival the final state's
own final weight is added. */
fn descend_to_final(
    t: &StdVectorFst,
    dist: &[Option<u32>],
    state: StateId,
    path: &mut HfstTwoLevelPath,
    rng: &mut Rng,
) {
    let mut s = state;
    loop {
        let d = dist[s as usize].expect("descend only enters co-accessible states");
        if d == 0 {
            // A final state: add its final weight and stop.
            path.first += *t
                .final_weight(s)
                .expect("s is a valid state of this fst")
                .expect("d == 0 marks a final state")
                .value();
            return;
        }
        // Collect the arcs that step strictly closer to a final state.
        let closer: Vec<StdTransition> = t
            .get_trs(s)
            .expect("s is a valid state of this fst")
            .trs()
            .iter()
            .filter(|arc| dist[arc.nextstate as usize] == Some(d - 1))
            .cloned()
            .collect();
        // `dist[s] == d > 0` guarantees at least one such arc exists.
        let arc = closer[(rng.next() as usize) % closer.len()].clone();
        path.second.push((
            crate::hfst_data_types::Symbol::new(
                t.input_symbols()
                    .expect("tropical transducer has an input symbol table")
                    .get_symbol(arc.ilabel)
                    .unwrap_or(""),
            ),
            crate::hfst_data_types::Symbol::new(
                t.input_symbols()
                    .expect("tropical transducer has an input symbol table")
                    .get_symbol(arc.olabel)
                    .unwrap_or(""),
            ),
        ));
        path.first += *arc.weight.value();
        s = arc.nextstate;
    }
}

/* Get a random path from transducer 't'. */
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.random-path-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.random-path-fn]
fn random_path_once(
    t: &StdVectorFst,
    dist: &[Option<u32>],
    rng: &mut Rng,
) -> Result<HfstTwoLevelPath, RandomPathError> {
    /* If the transducer is empty, return. */
    if is_minimal_and_empty(t) {
        return Err(RandomPathError::Empty);
    }

    let mut path = HfstTwoLevelPath {
        first: 0.0,
        second: StringPairVector::new(),
    };
    let mut current_state = t
        .start()
        .expect("start state present: non-empty checked above");

    let is_epsilon_path_accepted = t
        .is_final(current_state)
        .expect("start state is a valid state of this fst");

    let mut last_index: i32 = 0;
    // The weight the path must carry once truncated back to `last_index`:
    // the arc weights up to the last accepting prefix PLUS that state's own
    // final weight. Recomputed whenever `last_index` advances so the
    // truncation branch can restore the correct weight (upstream
    // hfst/hfst#441: the C++ truncation branch left `path.first` carrying the
    // arc weights of the popped tail and omitted the final-state weight).
    let mut last_weight: f32 = if is_epsilon_path_accepted {
        *t.final_weight(current_state)
            .expect("start state is a valid state of this fst")
            .expect("start confirmed final via is_epsilon_path_accepted")
            .value()
    } else {
        0.0
    };

    let num_states = t.num_states();
    let mut visited = vec![0i32; num_states];
    let mut broken = vec![0i32; num_states];

    loop {
        visited[current_state as usize] = 1;

        /* Only follow arcs whose target can still reach a final state; a
         * co-accessible non-final state therefore always has at least one
         * continuation, so the walk is guaranteed to reach a final state
         * before it runs out of moves (upstream hfst/hfst#444). */
        let mut t_transitions: Vec<StdTransition> = t
            .get_trs(current_state)
            .expect("current_state is a valid state of this fst")
            .trs()
            .iter()
            .filter(|arc| dist[arc.nextstate as usize].is_some())
            .cloned()
            .collect();

        /* If we cannot proceed, return the longest accepting prefix so far. */
        if t_transitions.is_empty() || broken[current_state as usize] != 0 {
            // If a final state was already passed, truncate back to it.
            if last_index > 0 || is_epsilon_path_accepted {
                let mut i = path.second.len() as i32 - 1;
                while i >= last_index {
                    path.second.pop();
                    i -= 1;
                }
                // Restore the weight of the truncated accepting prefix rather
                // than the overshot tail we just popped off.
                path.first = last_weight;
                return Ok(path);
            }
            // No accepting prefix yet: rather than abandon the walk (the
            // upstream hfst/hfst#444 failure), descend greedily along the
            // shortest route to a final state and return that accepting path.
            descend_to_final(t, dist, current_state, &mut path, rng);
            return Ok(path);
        }

        /* Pick one transition at random and proceed to its target; the
         * outer loop has already returned when there are none, so this
         * runs exactly once per state (it was a `while ... break` before). */
        if !t_transitions.is_empty() {
            let index = (rng.next() as usize) % t_transitions.len();
            let arc = t_transitions[index].clone();
            t_transitions.remove(index);

            let t_target = arc.nextstate;

            path.second.push((
                crate::hfst_data_types::Symbol::new(
                    t.input_symbols()
                        .expect("tropical transducer has an input symbol table")
                        .get_symbol(arc.ilabel)
                        .unwrap_or(""),
                ),
                crate::hfst_data_types::Symbol::new(
                    t.input_symbols()
                        .expect("tropical transducer has an input symbol table")
                        .get_symbol(arc.olabel)
                        .unwrap_or(""),
                ),
            ));
            path.first += *arc.weight.value();

            /* If the target state is final, */
            if t.is_final(t_target)
                .expect("t_target is a valid state of this fst")
            {
                if (rng.next() % 4) == 0 {
                    // randomly return the path so far,
                    path.first += *t
                        .final_weight(t_target)
                        .expect("t_target is a valid state of this fst")
                        .expect("t_target confirmed final via is_final")
                        .value();
                    if !is_epsilon_path_accepted && path.second.is_empty() {
                        return Err(RandomPathError::NoPath);
                    }
                    return Ok(path);
                } // or continue.
                last_index = path.second.len() as i32;
                // Remember the accepting weight at this prefix (arc weights so
                // far + this state's final weight) for the truncation branch.
                last_weight = path.first
                    + *t.final_weight(t_target)
                        .expect("t_target is a valid state of this fst")
                        .expect("t_target confirmed final via is_final")
                        .value();
            }

            /* Give more probability for shorter paths. */
            if broken[t_target as usize] == 0
                && visited[t_target as usize] == 1
                && (rng.next() % 4) == 0
            {
                broken[t_target as usize] = 1;
            }

            if visited[t_target as usize] == 1 && (rng.next() % 4) == 0 {
                broken[t_target as usize] = 1;
            }

            /* Proceed to the target state. */
            current_state = t_target;
        }
    }
}

/* Try to extract a random path from 't' at most 'max_times' times. */
fn random_path(
    t: &StdVectorFst,
    dist: &[Option<u32>],
    mut max_times: u32,
    rng: &mut Rng,
) -> Result<HfstTwoLevelPath, RandomPathError> {
    while max_times > 0 {
        max_times -= 1;
        match random_path_once(t, dist, rng) {
            Ok(p) => return Ok(p),
            Err(RandomPathError::Empty) => return Err(RandomPathError::Empty),
            Err(RandomPathError::NoPath) => continue,
        }
    }
    Err(RandomPathError::NoPath)
}

impl TropicalWeightTransducer {
    // ---- extract_paths / extract_random_paths --------------------------------

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-paths-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-paths-fn]
    pub fn extract_paths(
        t: &StdVectorFst,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        fd: Option<&FdTable<i64>>,
        filter_fd: bool,
    ) {
        if t.start().is_none() {
            return;
        }

        let all_visitations: BTreeMap<StateId, u16> = BTreeMap::new();
        let path_visitations: BTreeMap<StateId, u16> = BTreeMap::new();
        let mut fd_state_stack: Option<Vec<FdState<i64>>> = fd.map(|fd| vec![FdState::new(fd)]);

        let start = t
            .start()
            .expect("start state present: is_none checked above");
        let mut spv = StringPairVector::new();
        extract_paths(
            t,
            start,
            all_visitations,
            path_visitations,
            0.0f32,
            callback,
            cycles,
            fd_state_stack.as_mut(),
            filter_fd,
            &mut spv,
        );

        // add epsilon path, if needed
        if t.start().is_some()
            && t.is_final(start)
                .expect("start is a valid state of this fst")
        {
            let mut epsilon_path = HfstTwoLevelPath {
                first: *t
                    .final_weight(start)
                    .expect("start is a valid state of this fst")
                    .expect("start confirmed final via is_final")
                    .value(),
                second: StringPairVector::new(),
            };
            callback.operator_call(&mut epsilon_path, true /* final */);
        }
        // fd_state_stack dropped here (C++ 'delete fd_state_stack').
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fd-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fd-fn]
    pub fn extract_random_paths_fd(
        t: &StdVectorFst,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        filter_fd: bool,
    ) {
        let mut fdt = FlagDiacriticTable::new();
        let alpha = Self::get_alphabet(t);
        for it in alpha.iter() {
            fdt.insert_symbol(it);
        }

        let mut fd_results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        // We filter flags after extracting paths, so we request five times
        // more paths than wanted.
        Self::extract_random_paths(t, &mut fd_results, 5 * max_num);

        let mut max_num = max_num;
        for it in fd_results.iter() {
            if max_num <= 0 {
                break;
            }
            let mut path = it.clone();
            let sv = to_string_vector_from_path(&path);

            if fdt.is_valid_string(&sv) {
                if filter_fd {
                    path = remove_flags_two_level_path(&path);
                }
                results.insert(path);
                max_num -= 1;
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fn]
    pub fn extract_random_paths(t: &StdVectorFst, results: &mut HfstTwoLevelPaths, max_num: i32) {
        // Nanosecond granularity: the C++ seeded rand() from time(NULL)
        // (whole seconds), so every -r invocation within the same second
        // produced the identical "random" set. Seed from the finer clock.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let mut rng = Rng::seeded(seed);

        // Distance-to-final map: steers the random walk toward accepting
        // states so a non-empty transducer always yields paths (hfst/hfst#444).
        let dist = distance_to_final(t);

        let mut max_num = max_num;
        while max_num > 0 {
            /* Try to extract one path at most 5 times. */
            max_num -= 1;
            let mut path = match random_path(t, &dist, 5, &mut rng) {
                Ok(p) => p,
                Err(RandomPathError::NoPath) => {
                    continue; // one trial used, keep on trying
                }
                Err(RandomPathError::Empty) => {
                    return; // not even possible to extract paths
                }
            };

            /* If we extract the same path again, try at most 5 times to
            extract another one (a failed retry keeps the old path). */
            let mut i = max_num;
            while results.contains(&path) && i > 0 {
                i -= 1;
                if let Ok(p) = random_path(t, &dist, 5, &mut rng) {
                    path = p;
                } // keep on trying
            }

            /* Insert the path (another or the same). */
            results.insert(path);
        }
    }
}
