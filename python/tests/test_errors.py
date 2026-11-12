"""What a failed read or call raises, and what it leaves behind."""

from __future__ import annotations

import typing
from collections.abc import Callable
from pathlib import Path

import pytest

import divvun_hfst
from conftest import Fixtures

LOADERS: list[Callable[[Path], object]] = [
    divvun_hfst.load_transducers,
    divvun_hfst.load_transducer,
    divvun_hfst.load_optimised_transducer,
    divvun_hfst.load_tokeniser,
    divvun_hfst.load_dhfst,
]


# [spec:hfst:req:python.errors/test]
def test_the_exceptions_are_one_family() -> None:
    assert issubclass(divvun_hfst.HfstError, Exception)
    assert issubclass(divvun_hfst.FormatError, divvun_hfst.HfstError)
    assert divvun_hfst.HfstError.__module__ == "divvun_hfst"
    assert divvun_hfst.FormatError.__module__ == "divvun_hfst"
    assert typing.get_args(divvun_hfst.HfstErrorCode) == (
        "type-mismatch",
        "unsupported-format",
        "invalid-format",
        "io",
        "not-automaton",
        "cyclic",
        "invalid-argument",
        "internal",
        "failed",
        "transducer-count",
    )


# [spec:hfst:req:python.errors/test]
def test_readers_raise_format_error_with_a_code() -> None:
    readers: list[Callable[[bytes], object]] = [
        divvun_hfst.read_transducers,
        divvun_hfst.read_transducer,
        divvun_hfst.read_optimised_transducer,
        divvun_hfst.read_tokeniser,
        divvun_hfst.read_dhfst,
    ]
    for read in readers:
        with pytest.raises(divvun_hfst.FormatError) as raised:
            read(b"not anything the package reads")
        assert raised.value.code == "invalid-format"
        assert str(raised.value)


# [spec:hfst:req:python.errors/test]
def test_a_missing_file_raises_what_open_raises(tmp_path: Path) -> None:
    missing = tmp_path / "missing.hfstol"
    for load in LOADERS:
        with pytest.raises(FileNotFoundError) as raised:
            load(missing)
        assert raised.value.filename == str(missing)
    for load in LOADERS[2:]:
        with pytest.raises(OSError) as directory:
            load(tmp_path)
        assert directory.value.filename == str(tmp_path)


# [spec:hfst:req:python.errors/test]
# [spec:hfst:req:python.load+1/test]
def test_a_directory_not_thfst_is_refused(tmp_path: Path) -> None:
    for load in LOADERS[:2]:
        with pytest.raises(divvun_hfst.FormatError) as raised:
            load(tmp_path)
        assert raised.value.code == "invalid-format"
        assert "thfst" in str(raised.value)


# [spec:hfst:req:python.errors/test]
def test_a_failed_call_leaves_the_object_usable(fixtures: Fixtures) -> None:
    transducer = divvun_hfst.load_transducer(fixtures.path("words.optimized-lookup-weighted.hfst"))
    first = transducer.lookup("mouse")
    with pytest.raises(ValueError):
        transducer.lookup("mouse", limit=-1)
    assert transducer.lookup("mouse") == first
    tokeniser = divvun_hfst.load_tokeniser(fixtures.path("tokenise.pmhfst"))
    tokens = tokeniser.tokenise("a cat")
    with pytest.raises(ValueError):
        tokeniser.format("a cat", "nonsense")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        tokeniser.tokenise(b"a cat")  # type: ignore[arg-type]
    assert tokeniser.tokenise("a cat") == tokens


# [spec:hfst:req:python.errors/test]
def test_text_arguments_must_be_str(fixtures: Fixtures) -> None:
    transducer = divvun_hfst.load_transducer(fixtures.path("words.optimized-lookup-weighted.hfst"))
    with pytest.raises(TypeError):
        transducer.lookup(b"cat")  # type: ignore[arg-type]
    acceptor = divvun_hfst.load_dhfst(fixtures.path("speller.acceptor.dhfst"))
    assert isinstance(acceptor, divvun_hfst.DhfstAcceptor)
    with pytest.raises(TypeError):
        acceptor.lookup(None)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        divvun_hfst.load_transducer(3)  # type: ignore[arg-type]
