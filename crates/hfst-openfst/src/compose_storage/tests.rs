use super::trim::{
    MERGE_FAN_IN, ReverseEdge, TrimBudget, create_new_file, merge_reverse_runs,
    read_reverse_edge_opt, reverse_run_path, write_reverse_edge,
};
use super::*;
use rustfst::algorithms::connect;
use rustfst::fst_traits::{CoreFst, ExpandedFst};

fn sample_fst() -> Result<StdVectorFst> {
    let mut fst = StdVectorFst::new();
    let s0 = fst.add_state();
    let s1 = fst.add_state();
    let s2 = fst.add_state();
    fst.set_start(s0)?;
    fst.set_final(s1, TropicalWeight::new(2.5))?;
    fst.set_final(s2, TropicalWeight::new(-1.25))?;

    // Deliberately unsorted: the spool must retain within-state arc order.
    fst.add_tr(s0, StdTransition::new(7, 9, 0.75, s2))?;
    fst.add_tr(s0, StdTransition::new(3, 4, 1.5, s1))?;
    fst.add_tr(s1, StdTransition::new(5, 6, -0.5, s2))?;

    let mut input_symbols = SymbolTable::new();
    input_symbols.add_symbols(["i1", "i2", "i3"]);
    let mut output_symbols = SymbolTable::new();
    output_symbols.add_symbols(["o1", "o2", "o3"]);
    fst.set_input_symbols(Arc::new(input_symbols));
    fst.set_output_symbols(Arc::new(output_symbols));
    Ok(fst)
}

fn assert_preserved(expected: &StdVectorFst, actual: &StdVectorFst) -> Result<()> {
    assert_eq!(actual, expected);
    assert_eq!(actual.properties(), expected.properties());
    assert_eq!(actual.input_symbols(), expected.input_symbols());
    assert_eq!(actual.output_symbols(), expected.output_symbols());
    assert_eq!(actual.num_states(), expected.num_states());
    for state in expected.states_range() {
        assert_eq!(actual.get_trs(state)?.trs(), expected.get_trs(state)?.trs());
        assert_eq!(actual.final_weight(state)?, expected.final_weight(state)?);
    }
    Ok(())
}

fn connected(mut fst: StdVectorFst) -> Result<StdVectorFst> {
    connect(&mut fst)?;
    Ok(fst)
}

fn dead_branch_fst() -> Result<StdVectorFst> {
    let mut fst = StdVectorFst::new();
    let states: Vec<_> = (0..5).map(|_| fst.add_state()).collect();
    fst.set_start(states[0])?;
    fst.set_final(states[4], TropicalWeight::new(2.25))?;

    // Survivors are old IDs 0, 2, and 4. Their new IDs must therefore be
    // 0, 1, and 2, while this surviving subset retains its original order.
    fst.add_tr(states[0], StdTransition::new(90, 90, 9.0, states[3]))?;
    fst.add_tr(states[0], StdTransition::new(7, 8, 0.7, states[4]))?;
    fst.add_tr(states[0], StdTransition::new(3, 4, 0.3, states[2]))?;
    fst.add_tr(states[0], StdTransition::new(8, 9, 0.8, states[4]))?;
    fst.add_tr(states[0], StdTransition::new(91, 91, 9.1, states[1]))?;
    fst.add_tr(states[1], StdTransition::new(92, 92, 9.2, states[3]))?;
    fst.add_tr(states[3], StdTransition::new(93, 93, 9.3, states[1]))?;
    fst.add_tr(states[2], StdTransition::new(4, 5, 0.4, states[4]))?;
    fst.set_properties(
        fst.properties() & !(FstProperties::ACCESSIBLE | FstProperties::COACCESSIBLE),
    );
    Ok(fst)
}

#[test]
fn in_memory_materialization_preserves_order_and_metadata() -> Result<()> {
    let temp = TestDir::new()?;
    let source = sample_fst()?;
    let artifact = materialize_fst(
        &source,
        &ComposeStorageConfig::bounded(1024 * 1024, temp.path()),
    )?;
    assert!(!artifact.is_spilled());
    assert_preserved(&source, &artifact.into_vector_fst()?)
}

#[test]
fn unbounded_materialization_stays_in_memory() -> Result<()> {
    let temp = TestDir::new()?;
    let source = sample_fst()?;
    let artifact = materialize_fst(&source, &ComposeStorageConfig::unbounded(temp.path()))?;
    assert!(!artifact.is_spilled());
    assert_preserved(&source, &artifact.into_vector_fst()?)
}

#[test]
fn zero_limit_forces_spill_in_selected_directory() -> Result<()> {
    let temp = TestDir::new()?;
    let source = sample_fst()?;
    let expected = connected(source.clone())?;
    let mut artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(0, temp.path()))?;
    let ComposeArtifact::Scratch(spilled) = &artifact else {
        bail!("zero-byte tracked cap unexpectedly stayed in memory");
    };
    let scratch = spilled.scratch_dir().to_path_buf();
    assert_eq!(scratch.parent(), Some(temp.path()));
    assert!(
        scratch
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".hfst-compose."))
    );
    assert!(scratch.exists());
    assert_eq!(spilled.state_count(), 3);
    assert!(!artifact.is_externally_trimmed());
    assert!(artifact.prepare_for_reload()?);
    assert!(artifact.is_externally_trimmed());

    let actual = artifact.into_vector_fst()?;
    assert!(!scratch.exists());
    assert_preserved(&expected, &actual)
}

#[test]
fn spilled_trim_matches_connect_for_dead_cycle() -> Result<()> {
    let temp = TestDir::new()?;
    let source = dead_branch_fst()?;
    let expected = connected(source.clone())?;
    let mut artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(1, temp.path()))?;
    assert!(artifact.is_spilled());
    assert!(!artifact.is_externally_trimmed());
    assert!(artifact.prepare_for_reload()?);
    let ComposeArtifact::Scratch(spilled) = &artifact else {
        unreachable!("one-byte cap must spill this product")
    };
    assert_eq!(spilled.state_count(), 3);
    let actual = artifact.into_vector_fst()?;
    assert_preserved(&expected, &actual)?;
    assert_eq!(actual.get_trs(0)?.trs()[0].ilabel, 7);
    assert_eq!(actual.get_trs(0)?.trs()[0].nextstate, 2);
    assert_eq!(actual.get_trs(0)?.trs()[1].ilabel, 3);
    assert_eq!(actual.get_trs(0)?.trs()[1].nextstate, 1);
    assert_eq!(actual.get_trs(0)?.trs()[2].ilabel, 8);
    assert_eq!(actual.get_trs(0)?.trs()[2].nextstate, 2);
    Ok(())
}

#[test]
fn spilled_trim_preserves_some_semiring_zero_final() -> Result<()> {
    let temp = TestDir::new()?;
    let mut source = StdVectorFst::new();
    let s0 = source.add_state();
    let s1 = source.add_state();
    let s2 = source.add_state();
    source.set_start(s0)?;
    source.set_final(s1, TropicalWeight::zero())?;
    source.add_tr(s0, StdTransition::new(1, 1, 0.0, s1))?;
    source.add_tr(s0, StdTransition::new(2, 2, 0.0, s2))?;
    source.set_properties(
        source.properties() & !(FstProperties::ACCESSIBLE | FstProperties::COACCESSIBLE),
    );
    assert!(source.final_weight(s1)?.is_some());
    let expected = connected(source.clone())?;
    let artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(0, temp.path()))?;
    let actual = artifact.into_vector_fst()?;
    assert_preserved(&expected, &actual)?;
    assert_eq!(actual.final_weight(1)?, Some(TropicalWeight::zero()));
    Ok(())
}

#[test]
fn spilled_trim_without_finals_matches_connect_empty_result() -> Result<()> {
    let temp = TestDir::new()?;
    let mut source = StdVectorFst::new();
    let s0 = source.add_state();
    let s1 = source.add_state();
    source.set_start(s0)?;
    source.add_tr(s0, StdTransition::new(1, 1, 0.0, s1))?;
    source.set_properties(
        source.properties() & !(FstProperties::ACCESSIBLE | FstProperties::COACCESSIBLE),
    );
    let expected = connected(source.clone())?;
    let mut artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(0, temp.path()))?;
    assert!(artifact.prepare_for_reload()?);
    let ComposeArtifact::Scratch(spilled) = &artifact else {
        unreachable!("zero-byte cap must spill this product")
    };
    assert_eq!(spilled.state_count(), 0);
    let actual = artifact.into_vector_fst()?;
    assert_preserved(&expected, &actual)?;
    assert_eq!(actual.start(), None);
    Ok(())
}

#[test]
fn already_connected_property_path_is_a_verbatim_noop() -> Result<()> {
    let temp = TestDir::new()?;
    let source = connected(sample_fst()?)?;
    assert!(
        source
            .properties()
            .contains(FstProperties::ACCESSIBLE | FstProperties::COACCESSIBLE)
    );
    let mut artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(0, temp.path()))?;
    assert!(artifact.prepare_for_reload()?);
    assert_preserved(&source, &artifact.into_vector_fst()?)
}

#[test]
fn trim_error_is_contextual_and_cleans_scratch() -> Result<()> {
    let temp = TestDir::new()?;
    let source = dead_branch_fst()?;
    let mut artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(0, temp.path()))?;
    let ComposeArtifact::Scratch(spilled) = &artifact else {
        bail!("zero-byte tracked cap unexpectedly stayed in memory");
    };
    let scratch = spilled.scratch_dir().to_path_buf();
    let Some(SpilledData::Pending(pending)) = spilled.data.as_ref() else {
        bail!("new scratch artifact unexpectedly started trimmed");
    };
    fs::remove_file(&pending.spool_path)?;
    let error = artifact
        .prepare_for_reload()
        .expect_err("missing source spool must fail external trimming");
    let message = format!("{error:#}");
    assert!(message.contains("opening compose scratch spool"));
    assert!(message.contains("states.bin"));
    assert!(scratch.exists());
    drop(artifact);
    assert!(!scratch.exists());
    Ok(())
}

#[test]
fn multipass_reverse_merge_preserves_sorted_duplicates() -> Result<()> {
    let temp = TestDir::new()?;
    let scratch = ScratchDir::create(temp.path())?;
    let budget = TrimBudget {
        io_buffer_bytes: 1,
        sort_records: 1,
        page_cache_pages: 1,
        queue_records: 1,
    };
    let run_count = MERGE_FAN_IN + 3;
    let mut expected = Vec::new();
    for index in 0..run_count {
        let edge = ReverseEdge {
            target: StateId::try_from((run_count - index) % 7)?,
            source: StateId::try_from(index % 5)?,
        };
        expected.push(edge);
        let path = reverse_run_path(scratch.path(), 0, index);
        let mut file = create_new_file(&path, "test reverse-edge run")?;
        write_reverse_edge(&mut file, edge, &path)?;
    }
    expected.sort_unstable();
    let merged = merge_reverse_runs(scratch.path(), run_count, budget)?;
    let mut reader = BufReader::with_capacity(1, File::open(&merged)?);
    let mut actual = Vec::new();
    while let Some(edge) = read_reverse_edge_opt(&mut reader, &merged)? {
        actual.push(edge);
    }
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn scratch_is_removed_when_source_expansion_fails() -> Result<()> {
    let temp = TestDir::new()?;
    let mut malformed = StdVectorFst::new();
    let s0 = malformed.add_state();
    malformed.set_start(s0)?;
    malformed.add_tr(s0, StdTransition::new(1, 1, 0.0, 1))?;

    let error = materialize_fst(&malformed, &ComposeStorageConfig::bounded(0, temp.path()))
        .expect_err("missing target state must fail during expansion");
    assert!(error.to_string().contains("compose state 1"));
    assert_eq!(fs::read_dir(temp.path())?.count(), 0);
    Ok(())
}

#[test]
fn dropping_spilled_artifact_removes_scratch() -> Result<()> {
    let temp = TestDir::new()?;
    let source = sample_fst()?;
    let artifact = materialize_fst(&source, &ComposeStorageConfig::bounded(0, temp.path()))?;
    let ComposeArtifact::Scratch(spilled) = artifact else {
        bail!("zero-byte tracked cap unexpectedly stayed in memory");
    };
    let scratch = spilled.scratch_dir().to_path_buf();
    assert!(scratch.exists());
    drop(spilled);
    assert!(!scratch.exists());
    Ok(())
}

#[test]
fn invalid_scratch_parent_is_contextual_and_clean() -> Result<()> {
    let temp = TestDir::new()?;
    let parent_file = temp.path().join("not-a-directory");
    File::create(&parent_file)?;
    let source = sample_fst()?;
    let error = materialize_fst(&source, &ComposeStorageConfig::bounded(0, &parent_file))
        .expect_err("a file cannot contain a scratch directory");
    let message = format!("{error:#}");
    assert!(message.contains("creating compose scratch directory"));
    assert!(message.contains("not-a-directory"));
    assert_eq!(fs::read_dir(temp.path())?.count(), 1);
    Ok(())
}

#[test]
fn unbounded_plan_and_storage_remain_explicitly_unbounded() {
    let plan = ComposeMemoryPlan::from_allowance(None);
    assert_eq!(plan, ComposeMemoryPlan::Unbounded);
    assert_eq!(plan.allowance_bytes(), None);
    assert_eq!(plan.tracked_cap_bytes(), None);
    assert_eq!(plan.pair_interner_cap_bytes(), None);
    assert_eq!(plan.materializer_cap_bytes(), None);

    let storage = ComposeStorageConfig::unbounded(".");
    assert_eq!(storage.tracked_memory_cap_bytes, None);
}

#[test]
fn bounded_plan_reserves_once_and_partitions_without_overflow() {
    let maximum_tracked = u64::MAX / 10 * 9 + u64::MAX % 10 * 9 / 10;
    for (allowance, expected_tracked) in [
        (0, 0),
        (1, 0),
        (9, 8),
        (10, 9),
        (11, 9),
        (19, 17),
        (20, 18),
        (u64::MAX, maximum_tracked),
    ] {
        let plan = ComposeMemoryPlan::from_allowance(Some(allowance));
        let ComposeMemoryPlan::Bounded {
            allowance_bytes,
            tracked_cap_bytes,
            pair_interner_cap_bytes,
            materializer_cap_bytes,
        } = plan
        else {
            panic!("bounded allowance produced an unbounded plan");
        };

        assert_eq!(allowance_bytes, allowance);
        assert_eq!(tracked_cap_bytes, expected_tracked);
        assert_eq!(materializer_cap_bytes, tracked_cap_bytes / 4);
        assert_eq!(
            pair_interner_cap_bytes,
            tracked_cap_bytes - materializer_cap_bytes
        );
        assert_eq!(
            pair_interner_cap_bytes.checked_add(materializer_cap_bytes),
            Some(tracked_cap_bytes)
        );
        assert!(tracked_cap_bytes <= allowance_bytes);
        assert_eq!(plan.allowance_bytes(), Some(allowance_bytes));
        assert_eq!(plan.tracked_cap_bytes(), Some(tracked_cap_bytes));
        assert_eq!(
            plan.pair_interner_cap_bytes(),
            Some(pair_interner_cap_bytes)
        );
        assert_eq!(plan.materializer_cap_bytes(), Some(materializer_cap_bytes));
    }
}

#[test]
fn bounded_storage_uses_the_derived_cap_verbatim() -> Result<()> {
    let temp = TestDir::new()?;
    let source = sample_fst()?;
    let mut memory = MemoryBuffer::default();
    for state in source.states_range() {
        let trs = source.get_trs(state)?;
        assert!(memory.try_append(source.final_weight(state)?, trs.trs(), None)?);
    }
    let exact_cap = bytes_for::<BufferedState>(memory.states.capacity())?
        + bytes_for::<StdTransition>(memory.arc_capacity)?
        + bytes_for::<BufferedState>(memory.states.len())?;

    let exact = ComposeStorageConfig::bounded(exact_cap, temp.path());
    assert_eq!(exact.tracked_memory_cap_bytes, Some(exact_cap));
    assert!(!materialize_fst(&source, &exact)?.is_spilled());

    let one_byte_short = ComposeStorageConfig::bounded(exact_cap - 1, temp.path());
    assert!(materialize_fst(&source, &one_byte_short)?.is_spilled());
    Ok(())
}

#[derive(Debug)]
struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Result<Self> {
        for _ in 0..128 {
            let sequence = SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "hfst-compose-storage-test-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).context("creating compose storage test directory");
                }
            }
        }
        bail!("could not create compose storage test directory")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _cleanup_result = fs::remove_dir_all(&self.0);
    }
}
