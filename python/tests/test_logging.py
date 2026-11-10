"""The library's events reach Python's logging module."""

from __future__ import annotations

import logging

import pytest

import divvun_hfst
from conftest import Fixtures


def unhandled_symbol(fixtures: Fixtures) -> bytes:
    """A transducer whose alphabet holds `@odd1@`, which divvun-fst warns it
    does not know. `+Event` has as many bytes, so the tables still read."""
    data = fixtures.bytes("identity.optimized-lookup-weighted.hfst")
    assert data.count(b"\0+Event\0") == 1
    return data.replace(b"\0+Event\0", b"\0@odd1@\0")


def modifier_tag(fixtures: Fixtures) -> divvun_hfst.Tokeniser:
    """The tokenise fixture with its tag `+CLB` spelt `ŋʷ`, a letter and a
    modifier letter, which the hfst library warns about when it prints the
    tag. Both spellings are four bytes, so the tables still read."""
    data = fixtures.bytes("tokenise.pmhfst")
    assert data.count(b"\0+CLB\0") == 1
    return divvun_hfst.read_tokeniser(data.replace(b"\0+CLB\0", "\0ŋʷ\0".encode()))


# [spec:hfst:req:python.logging/test]
def test_hfst_events_reach_their_targets_logger(
    fixtures: Fixtures, caplog: pytest.LogCaptureFixture
) -> None:
    tokeniser = modifier_tag(fixtures)
    with caplog.at_level(logging.DEBUG, logger="hfst"):
        tokeniser.format("a cat.", "giella-cg")
    [record] = caplog.records
    assert record.name == "hfst.pmatch_tokenize"
    assert record.levelno == logging.WARNING
    assert "ŋʷ" in record.getMessage()


# [spec:hfst:req:python.logging/test]
def test_divvun_fst_events_go_under_divvun_fst(
    fixtures: Fixtures, caplog: pytest.LogCaptureFixture
) -> None:
    with caplog.at_level(logging.WARNING, logger="divvun_fst"):
        divvun_hfst.read_optimised_transducer(unhandled_symbol(fixtures))
    [record] = caplog.records
    assert record.name == "divvun_fst.transducer.hfst.alphabet"
    assert record.levelno == logging.WARNING
    assert "@odd1@" in record.getMessage()


# [spec:hfst:req:python.logging/test]
def test_each_root_keeps_its_own_level(
    fixtures: Fixtures, caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(logging.DEBUG, logger="hfst")
    caplog.set_level(logging.ERROR, logger="divvun_fst")
    divvun_hfst.read_optimised_transducer(unhandled_symbol(fixtures))
    assert caplog.records == []

    caplog.set_level(logging.ERROR, logger="hfst")
    caplog.set_level(logging.WARNING, logger="divvun_fst")
    modifier_tag(fixtures).format("a cat.", "giella-cg")
    assert caplog.records == []


# [spec:hfst:req:python.logging/test]
def test_logging_disable_keeps_nothing(
    fixtures: Fixtures, caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(5, logger="hfst")
    caplog.set_level(5, logger="divvun_fst")
    logging.disable(logging.CRITICAL)
    try:
        divvun_hfst.read_optimised_transducer(unhandled_symbol(fixtures))
        modifier_tag(fixtures).format("a cat.", "giella-cg")
    finally:
        logging.disable(logging.NOTSET)
    assert caplog.records == []
