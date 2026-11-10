"""What every test reads: the transducers, tokenisers and DHFST files the npm
package's tests read, and the native library's results for them.

They come from the directory `DIVVUN_HFST_FIXTURES` names, such as the
target/js-test/fixtures `node js/build.mjs --test` writes, or else
`node js/test/fixtures.mjs` builds them, with this checkout's hfst tools,
into a directory of the session's own.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[2]


class Fixtures:
    """A directory of built fixtures."""

    def __init__(self, directory: Path) -> None:
        self.directory = directory

    def path(self, name: str) -> Path:
        return self.directory / name

    def bytes(self, name: str) -> bytes:
        return self.path(name).read_bytes()

    def text(self, name: str) -> str:
        return self.path(name).read_text(encoding="utf-8")

    def json(self, name: str) -> Any:
        return json.loads(self.text(name))


@pytest.fixture(scope="session")
def fixtures(tmp_path_factory: pytest.TempPathFactory) -> Fixtures:
    given = os.environ.get("DIVVUN_HFST_FIXTURES")
    if given:
        directory = Path(given)
        assert (directory / "parity.json").is_file(), (
            f"DIVVUN_HFST_FIXTURES names {given}, which holds no built fixtures"
        )
        return Fixtures(directory)
    directory = tmp_path_factory.mktemp("fixtures")
    subprocess.run(
        ["node", ROOT / "js" / "test" / "fixtures.mjs", directory],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    return Fixtures(directory)


READ = ("optimized-lookup-weighted", "optimized-lookup-unweighted")
"""The transducer types the package reads, as the fixtures name them."""
