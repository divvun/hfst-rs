"""Reading transducers, tokenisers and DHFST files from bytes and from paths."""

from __future__ import annotations


import pytest

import divvun_hfst
from conftest import READ, Fixtures


# [spec:hfst:req:python.load+1/test]
def test_a_stream_reads_in_stream_order(fixtures: Fixtures) -> None:
    transducers = divvun_hfst.read_transducers(fixtures.bytes("ordered.hfst"))
    assert [t.name for t in transducers] == ["first", "second"]
    assert all(t.type == "optimized-lookup-weighted" for t in transducers)
    bare = divvun_hfst.read_transducers(fixtures.bytes("ordered.bare"))
    assert [t.name for t in bare] == ["", ""]
    assert [t.properties for t in bare] == [{}, {}]


# [spec:hfst:req:python.load+1/test]
def test_both_types_read_with_and_without_headers(fixtures: Fixtures) -> None:
    for kind in READ:
        for suffix in ("hfst", "bare"):
            transducer = divvun_hfst.read_transducer(fixtures.bytes(f"words.{kind}.{suffix}"))
            assert transducer.type == kind
            assert [r.output for r in transducer.lookup("cat")] == ["dog"]
            optimised = divvun_hfst.read_optimised_transducer(
                fixtures.bytes(f"words.{kind}.{suffix}")
            )
            assert optimised.type == kind
            assert [r.output for r in optimised.lookup("cat")] == ["dog"]


# [spec:hfst:req:python.load+1/test]
def test_properties_leave_out_version_type_and_name(fixtures: Fixtures) -> None:
    transducer = divvun_hfst.read_transducer(fixtures.bytes("first.hfst"))
    assert transducer.name == "first"
    assert not {"version", "type", "name"} & set(transducer.properties)
    transducer.properties["added"] = "here"
    assert "added" not in transducer.properties
    container = divvun_hfst.read_transducers(fixtures.bytes("animals.pmhfst"))
    assert container[0].name == "TOP"
    assert {t.name for t in container[1:]} == {"Animal"}


# [spec:hfst:req:python.load+1/test]
def test_bytes_and_bytearray_both_read(fixtures: Fixtures) -> None:
    data = fixtures.bytes("tokenise.pmhfst")
    for given in (data, bytearray(data)):
        assert divvun_hfst.read_tokeniser(given).tokenise("cat")[0].surface == "cat"
    with pytest.raises(TypeError):
        divvun_hfst.read_transducers("HFST")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        divvun_hfst.read_dhfst(memoryview(data))  # type: ignore[arg-type]


# [spec:hfst:req:python.load+1/test]
def test_every_load_takes_str_and_pathlike(fixtures: Fixtures) -> None:
    for path in (fixtures.path("first.hfst"), str(fixtures.path("first.hfst"))):
        assert divvun_hfst.load_transducer(path).name == "first"
        assert [t.name for t in divvun_hfst.load_transducers(path)] == ["first"]
        assert divvun_hfst.load_optimised_transducer(path).name == "first"
    tokeniser = divvun_hfst.load_tokeniser(fixtures.path("tokenise.pmhfst"))
    assert [t.surface for t in tokeniser.tokenise("a cat")] == ["a", "cat"]
    acceptor = divvun_hfst.load_dhfst(str(fixtures.path("speller.acceptor.dhfst")))
    assert acceptor.type == "acceptor"


# [spec:hfst:req:python.load+1/test]
def test_one_transducer_readers_refuse_several(fixtures: Fixtures) -> None:
    with pytest.raises(divvun_hfst.FormatError, match="holds 2") as raised:
        divvun_hfst.read_transducer(fixtures.bytes("ordered.hfst"))
    assert raised.value.code == "transducer-count"
    with pytest.raises(divvun_hfst.FormatError) as raised:
        divvun_hfst.load_transducer(fixtures.path("ordered.bare"))
    assert raised.value.code == "transducer-count"
    assert "load_transducer" in str(raised.value)
    with pytest.raises(divvun_hfst.FormatError) as raised:
        divvun_hfst.read_optimised_transducer(fixtures.bytes("animals.pmhfst"))
    assert raised.value.code == "transducer-count"


# [spec:hfst:req:python.load+1/test]
def test_every_type_reads(fixtures: Fixtures) -> None:
    cases = {
        "words.openfst-tropical.hfst": "openfst-tropical",
        "words.openfst-tropical.bare": "openfst-tropical",
        "words.foma.hfst": "foma",
        "words.foma.plain": "foma",
        "words.optimized-lookup-weighted.hfst": "optimized-lookup-weighted",
        "words.optimized-lookup-unweighted.bare": "optimized-lookup-unweighted",
    }
    for name, kind in cases.items():
        [transducer] = divvun_hfst.read_transducers(fixtures.bytes(name))
        assert transducer.type == kind, name
        assert [r.output for r in transducer.lookup("cat")] == ["dog"], name
        assert sorted(r.output for r in transducer.lookup("mouse")) == ["mice", "mouse"], name


# [spec:hfst:req:python.load+1/test]
def test_a_thfst_directory_loads(fixtures: Fixtures) -> None:
    for path in (fixtures.path("words.thfst"), str(fixtures.path("words.thfst"))):
        [transducer] = divvun_hfst.load_transducers(path)
        assert transducer.type == "optimized-lookup-weighted"
        assert [r.output for r in transducer.lookup("cat")] == ["dog"]
        assert divvun_hfst.load_transducer(path).lookup("cat") == transducer.lookup("cat")


# [spec:hfst:req:python.load+1/test]
def test_other_readers_name_formats_they_refuse(fixtures: Fixtures) -> None:
    cases = {
        "words.openfst-tropical.hfst": "openfst-tropical",
        "words.openfst-tropical.bare": "openfst-tropical",
        "words.foma.hfst": "foma",
        "words.foma.bare": "gzip",
        "words.foma.plain": "foma",
    }
    for name, named in cases.items():
        with pytest.raises(divvun_hfst.FormatError) as raised:
            divvun_hfst.read_optimised_transducer(fixtures.bytes(name))
        assert raised.value.code == "unsupported-format", name
        assert named in str(raised.value), name
        assert "divvun-hfst does not read" in str(raised.value), name
    with pytest.raises(divvun_hfst.FormatError) as raised:
        divvun_hfst.read_transducers(fixtures.bytes("words.foma.bare"))
    assert raised.value.code == "unsupported-format"
    assert "gzip" in str(raised.value)


# [spec:hfst:req:python.load+1/test]
def test_mixed_and_broken_streams_are_refused(fixtures: Fixtures) -> None:
    with pytest.raises(divvun_hfst.FormatError) as raised:
        divvun_hfst.read_transducers(fixtures.bytes("mixed.hfst"))
    assert raised.value.code == "type-mismatch"
    data = fixtures.bytes("words.optimized-lookup-weighted.hfst")
    for broken in (b"", data[:-9], b"not a transducer stream"):
        with pytest.raises(divvun_hfst.FormatError) as raised:
            divvun_hfst.read_transducers(broken)
        assert raised.value.code == "invalid-format"


# [spec:hfst:req:python.load+1/test]
def test_dhfst_is_not_a_transducer_stream(fixtures: Fixtures) -> None:
    with pytest.raises(divvun_hfst.FormatError, match="DHFST") as raised:
        divvun_hfst.read_transducers(fixtures.bytes("speller.acceptor.dhfst"))
    assert raised.value.code == "unsupported-format"
    with pytest.raises(divvun_hfst.FormatError, match="read_transducers reads"):
        divvun_hfst.read_dhfst(fixtures.bytes("words.optimized-lookup-weighted.hfst"))


# [spec:hfst:req:python.load+1/test]
def test_a_tokeniser_reads_only_pmatch_containers(fixtures: Fixtures) -> None:
    for name in ("words.optimized-lookup-weighted.hfst", "speller.acceptor.dhfst"):
        with pytest.raises(divvun_hfst.FormatError) as raised:
            divvun_hfst.read_tokeniser(fixtures.bytes(name))
        assert raised.value.code in ("invalid-format", "unsupported-format")
    with pytest.raises(divvun_hfst.FormatError, match="naive tokeniser"):
        divvun_hfst.read_tokeniser(fixtures.bytes("words.optimized-lookup-weighted.hfst"))


# [spec:hfst:req:python.load+1/test]
def test_objects_come_only_from_readers() -> None:
    for cls in (
        divvun_hfst.Transducer,
        divvun_hfst.OptimisedTransducer,
        divvun_hfst.Tokeniser,
        divvun_hfst.DhfstAcceptor,
        divvun_hfst.DhfstErrorModel,
        divvun_hfst.LookupResult,
        divvun_hfst.Token,
    ):
        with pytest.raises(TypeError):
            cls()


def test_reprs_name_the_class_and_type(fixtures: Fixtures) -> None:
    named = divvun_hfst.read_transducer(fixtures.bytes("first.hfst"))
    assert repr(named) == "<divvun_hfst.Transducer optimized-lookup-weighted 'first'>"
    bare = divvun_hfst.read_optimised_transducer(
        fixtures.bytes("words.optimized-lookup-unweighted.bare")
    )
    assert repr(bare) == "<divvun_hfst.OptimisedTransducer optimized-lookup-unweighted>"
