"""Loads and calls release the GIL, and threads share one loaded object."""

from __future__ import annotations

import threading
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor

import divvun_hfst
from conftest import Fixtures

BUSY = 1000
"""The fewest turns a Python loop takes beside work that lets it run."""


def python_runs_during(work: Callable[[], object]) -> int:
    """How many times the calling thread gets round a Python loop while `work`
    runs in another thread. Work that holds the GIL from start to finish leaves
    the loop next to no turns."""
    started = threading.Event()

    def run() -> None:
        started.set()
        work()

    worker = threading.Thread(target=run)
    worker.start()
    started.wait()
    turns = 0
    while worker.is_alive():
        turns += 1
    worker.join()
    return turns


def long_text(fixtures: Fixtures) -> str:
    line: str = fixtures.json("tokenise-cases.json")["texts"]["line"]
    return line * 400


# [spec:hfst:req:python.threads/test]
def test_a_lookup_releases_the_gil(fixtures: Fixtures) -> None:
    many = divvun_hfst.load_transducer(fixtures.path("many.optimized-lookup-weighted.hfst"))
    assert python_runs_during(lambda: [many.lookup("aaaaaaa") for _ in range(5)]) > BUSY


# [spec:hfst:req:python.threads/test]
def test_an_optimised_lookup_releases_the_gil(fixtures: Fixtures) -> None:
    many = divvun_hfst.load_optimised_transducer(
        fixtures.path("many.optimized-lookup-weighted.hfst")
    )
    assert python_runs_during(lambda: [many.lookup("aaaaaaa") for _ in range(5)]) > BUSY


# [spec:hfst:req:python.threads/test]
def test_tokenising_releases_the_gil(fixtures: Fixtures) -> None:
    tokeniser = divvun_hfst.load_tokeniser(fixtures.path("tokenise.pmhfst"))
    text = long_text(fixtures)
    assert python_runs_during(lambda: tokeniser.tokenise(text)) > BUSY
    assert python_runs_during(lambda: tokeniser.format(text, "cg")) > BUSY


# [spec:hfst:req:python.threads/test]
def test_a_load_releases_the_gil(fixtures: Fixtures) -> None:
    stream = fixtures.bytes("ordered.bare") * 2000
    assert python_runs_during(lambda: divvun_hfst.read_transducers(stream)) > BUSY


# [spec:hfst:req:python.threads/test]
def test_threads_share_one_loaded_object(fixtures: Fixtures) -> None:
    tokeniser = divvun_hfst.load_tokeniser(fixtures.path("tokenise.pmhfst"))
    texts = list(fixtures.json("tokenise-cases.json")["texts"].values())
    transducer = divvun_hfst.load_transducer(fixtures.path("flags.optimized-lookup-weighted.hfst"))
    optimised = divvun_hfst.load_optimised_transducer(
        fixtures.path("flags.optimized-lookup-weighted.hfst")
    )
    acceptor = divvun_hfst.load_dhfst(fixtures.path("speller.acceptor.dhfst"))
    assert isinstance(acceptor, divvun_hfst.DhfstAcceptor)
    words = fixtures.text("speller-inputs.txt").splitlines()

    def everything(i: int) -> object:
        text = texts[i % len(texts)]
        return (
            tokeniser.tokenise(text),
            tokeniser.format(text, "giella-cg"),
            transducer.lookup("talo"),
            optimised.lookup("talo"),
            [acceptor.lookup(w) for w in words],
        )

    alone = [everything(i) for i in range(len(texts))]
    with ThreadPoolExecutor(max_workers=8) as pool:
        together = list(pool.map(everything, range(64)))
    assert together == [alone[i % len(texts)] for i in range(64)]
