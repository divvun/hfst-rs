"""DHFST files: acceptors to look words up in, and error models."""

from __future__ import annotations

import pytest

import divvun_hfst
from conftest import Fixtures


def acceptor(fixtures: Fixtures, name: str) -> divvun_hfst.DhfstAcceptor:
    model = divvun_hfst.load_dhfst(fixtures.path(name))
    assert isinstance(model, divvun_hfst.DhfstAcceptor)
    return model


# [spec:hfst:req:python.dhfst/test]
def test_an_acceptor_describes_itself(fixtures: Fixtures) -> None:
    speller = acceptor(fixtures, "speller.acceptor.dhfst")
    assert speller.type == "acceptor"
    assert speller.version == 1
    assert isinstance(speller.symbols, tuple)
    assert speller.symbols[0] == "@_EPSILON_SYMBOL_@"
    assert speller.meta is None or speller.meta.startswith("{")
    info = speller.info
    assert isinstance(info, divvun_hfst.AcceptorInfo)
    assert info.slots >= info.ids + info.label_max
    assert info.arcs >= info.free_arcs
    assert isinstance(info.distances, bool)
    assert info == speller.info
    assert repr(info).startswith(f"AcceptorInfo(ids={info.ids}, slots={info.slots},")


# [spec:hfst:req:python.dhfst/test]
def test_an_acceptor_reads_words_as_graphemes(fixtures: Fixtures) -> None:
    graphemes = acceptor(fixtures, "graphemes.acceptor.dhfst")
    assert graphemes.lookup("ija") == 2.0, "i, j and a, not the symbol ij"
    assert graphemes.lookup("é") == 0.25
    assert graphemes.lookup("é") is None, "only the composed form is a symbol"
    assert graphemes.lookup("xy") is None, "xy is one symbol, but two graphemes"
    assert graphemes.lookup("") is None


# [spec:hfst:req:python.dhfst/test]
def test_an_error_model_describes_itself(fixtures: Fixtures) -> None:
    model = divvun_hfst.read_dhfst(fixtures.bytes("errmodel.error-model.dhfst"))
    assert isinstance(model, divvun_hfst.DhfstErrorModel)
    assert model.type == "error-model"
    assert model.version == 1
    assert model.state_count > 0
    assert model.symbols[0] == "@_EPSILON_SYMBOL_@"
    assert not hasattr(model, "lookup")


# [spec:hfst:req:python.dhfst/test]
def test_bytes_that_are_not_dhfst_are_refused(fixtures: Fixtures) -> None:
    whole = fixtures.bytes("speller.acceptor.dhfst")
    for broken in (whole[:-9], whole[:6], b"", b"not a speller"):
        with pytest.raises(divvun_hfst.FormatError) as raised:
            divvun_hfst.read_dhfst(broken)
        assert raised.value.code == "invalid-format"
    with pytest.raises(divvun_hfst.FormatError) as raised:
        divvun_hfst.read_dhfst(fixtures.bytes("words.foma.hfst"))
    assert raised.value.code == "unsupported-format"
