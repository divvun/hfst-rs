"""The writers: a transducer's bytes in every type, its text, DHFST and
THFST, and BHFST spellers."""

from __future__ import annotations

from pathlib import Path

import pytest

import divvun_hfst
from conftest import Fixtures
from test_threads import BUSY, python_runs_during

TYPES: list[divvun_hfst.TransducerType] = [
    "openfst-tropical",
    "foma",
    "optimized-lookup-weighted",
    "optimized-lookup-unweighted",
]


def paths(t: divvun_hfst.Transducer) -> list[tuple[str, str]]:
    return sorted((p.input, p.output) for p in t.extract_paths())


# [spec:hfst:req:python.writers/test]
def test_to_bytes_writes_every_type(fixtures: Fixtures) -> None:
    for kind in TYPES:
        name = f"words.{kind}.hfst"
        t = divvun_hfst.read_transducer(fixtures.bytes(name))
        assert t.to_bytes() == fixtures.bytes(name), name
        for to in TYPES:
            written = t.to_bytes(to)
            assert divvun_hfst.read_transducer(written).type == to, (name, to)
        bare = t.to_bytes(use_backend_format=True)
        assert not bare.startswith(b"HFST\0")
        if kind == "foma":
            assert bare.startswith(b"\x1f\x8b"), "foma's own format is gzipped"
        else:
            assert divvun_hfst.read_transducer(bare).type == kind
    tropical = divvun_hfst.read_transducer(fixtures.bytes("words.openfst-tropical.hfst"))
    assert tropical.to_bytes("optimized-lookup-weighted", quick=True)
    with pytest.raises(ValueError):
        tropical.to_bytes("sfst")  # type: ignore[arg-type]


# [spec:hfst:req:python.writers/test]
def test_text_is_what_fst2txt_writes(fixtures: Fixtures) -> None:
    t = divvun_hfst.regex("{ab}:{c}::0.5")
    assert t.to_att() == "0\t1\ta\tc\t0.500000\n1\t2\tb\t@0@\t0.000000\n2\t0.000000\n"
    assert t.to_att(do_not_print_weights=True) == "0\t1\ta\tc\n1\t2\tb\t@0@\n2\n"
    numbers = t.to_att(use_numbers=True)
    assert set(numbers) <= set("0123456789.\t\n")
    prolog = t.to_prolog()
    assert prolog.startswith("network(NO_NAME_1).\n")
    assert 'arc(NO_NAME_1, 0, 1, "a":"c", 0.500000).' in prolog
    assert t.to_dot().startswith("// This graph generated with hfst-fst2txt\n")
    assert isinstance(t.to_pckimmo(), str)
    foma = divvun_hfst.read_transducer(fixtures.bytes("words.foma.hfst"))
    assert "\t0.000000" not in foma.to_att()
    assert "\t0.000000" in foma.to_att(print_weights=True)


# [spec:hfst:req:python.writers/test]
def test_dhfst_acceptors_and_error_models() -> None:
    lexicon = divvun_hfst.regex("{cat} | {dog}")
    acceptor = divvun_hfst.read_dhfst(lexicon.to_dhfst_acceptor(source_name="words.hfst"))
    assert isinstance(acceptor, divvun_hfst.DhfstAcceptor)
    assert acceptor.lookup("cat") == 0.0 and acceptor.lookup("cow") is None
    assert acceptor.meta is not None and '"source":"words.hfst"' in acceptor.meta
    errors = divvun_hfst.regex("[a:b | b:a | a | b]*")
    for depth in (4, 1, None):
        model = divvun_hfst.read_dhfst(errors.to_dhfst_error_model(max_fallback_depth=depth))
        assert isinstance(model, divvun_hfst.DhfstErrorModel)
    with pytest.raises(divvun_hfst.HfstError):
        divvun_hfst.regex("{cat}:{dog}").to_dhfst_acceptor()


# [spec:hfst:req:python.writers/test]
def test_write_thfst_writes_the_directory(tmp_path: Path) -> None:
    t = divvun_hfst.regex("{cat}:{dog}::0.5")
    directory = tmp_path / "words.thfst"
    t.write_thfst(directory)
    assert sorted(p.name for p in directory.iterdir()) == ["alphabet", "index", "transition"]
    [read] = divvun_hfst.load_transducers(directory)
    assert [(r.output, r.weight) for r in read.lookup("cat")] == [("dog", 0.5)]
    t.write_thfst(str(tmp_path / "quick.thfst"), quick=True)
    (tmp_path / "a file").write_bytes(b"")
    with pytest.raises(divvun_hfst.HfstError):
        t.write_thfst(tmp_path / "a file" / "x.thfst")


# [spec:hfst:req:python.writers/test]
def test_pack_bhfst_packs_a_speller() -> None:
    lexicon = divvun_hfst.regex("{cat} | {dog}")
    errors = divvun_hfst.regex("[a:b | b:a | a | b]*").to_dhfst_error_model()
    for acceptor in (lexicon.to_dhfst_acceptor(), lexicon.to_bytes()):
        packed = divvun_hfst.pack_bhfst(acceptor, errors, '{"id": "x"}')
        assert packed.startswith(b"\xffBOX")
    configured = divvun_hfst.pack_bhfst(
        lexicon.to_dhfst_acceptor(), bytearray(errors), b'{"id": "x"}',
        speller_config='{"beam": 3}',
    )
    assert configured != divvun_hfst.pack_bhfst(lexicon.to_dhfst_acceptor(), errors, '{"id": "x"}')
    with pytest.raises(divvun_hfst.HfstError) as raised:
        divvun_hfst.pack_bhfst(lexicon.to_dhfst_acceptor(), errors, speller_config="{}")
    assert raised.value.code == "invalid-argument"
    with pytest.raises(divvun_hfst.HfstError):
        divvun_hfst.pack_bhfst(errors, errors)
    with pytest.raises(TypeError):
        divvun_hfst.pack_bhfst(lexicon.to_bytes(), errors, 1)  # type: ignore[arg-type]


# [spec:hfst:req:python.writers/test]
def test_writing_releases_the_gil(fixtures: Fixtures) -> None:
    many = divvun_hfst.load_transducer(fixtures.path("many.openfst-tropical.hfst"))
    assert python_runs_during(
        lambda: [many.to_bytes("optimized-lookup-weighted") for _ in range(5)]
    ) > BUSY
