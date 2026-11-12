"""What the compile parity suites share: the package's side of each case of
compile-parity.json, and the check of it against the tool's.

js/test/compile-fixtures.mjs runs the tools on the inputs the repository's
tests give them, the inputs and cases of crates/hfst-cli/tests/embed_parity.rs,
and writes compile-parity.json beside the fixtures: each case's call as this
package names it, the tool's arguments, whether the tool succeeded, and what
it wrote. The suites run each case through the built package. A compiler,
operation or writer gives the tool's bytes or text, byte for byte; a query
gives data, which is printed as the tool prints it and compared with what
the tool printed. Where the tool failed, the package raises.
"""

from __future__ import annotations

import decimal
import math
import struct
from collections.abc import Callable
from typing import Any

import divvun_hfst
from conftest import Fixtures

Transducer = divvun_hfst.Transducer
Case = dict[str, Any]


def f32(x: float) -> float:
    rounded: float = struct.unpack("f", struct.pack("f", x))[0]
    return rounded


def shown(weight: float) -> str:
    """A weight as the tools print a 32-bit float on its own: the fewest
    digits that read back as it, never with an exponent."""
    if math.isinf(weight):
        return "inf" if weight > 0 else "-inf"
    digits = repr(weight)
    for precision in range(1, 18):
        digits = f"{weight:.{precision}g}"
        if f32(float(digits)) == weight:
            break
    text = format(decimal.Decimal(digits), "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text


class Runner:
    """The package's side of each case, from the fixtures' files."""

    def __init__(self, fixtures: Fixtures, manifest: dict[str, Any]) -> None:
        self.fixtures = fixtures
        self.inputs: dict[str, str] = manifest["inputs"]

    def all(self, name: str) -> list[Transducer]:
        return divvun_hfst.read_transducers(self.fixtures.bytes(self.inputs[name]))

    def one(self, name: str) -> Transducer:
        [t] = self.all(name)
        return t

    def value(self, given: Any) -> Any:
        """A value of the manifest as the call takes it: a transducer, a
        file's text or an infinity where it names one."""
        if isinstance(given, dict):
            if "$transducer" in given:
                return self.one(given["$transducer"])
            if "$text" in given:
                return self.fixtures.text(given["$text"])
            if "$float" in given:
                return float(given["$float"])
            return {key: self.value(v) for key, v in given.items()}
        return given

    def call(self, target: object, method: str, kwargs: dict[str, Any]) -> Any:
        given = dict(kwargs)
        args = given.pop("$args", [])
        resolved = {key: self.value(v) for key, v in given.items()}
        return getattr(target, method)(*args, **resolved)

    def expected(self, case: Case) -> bytes:
        return self.fixtures.bytes(case["expected"])

    def printed(self, case: Case) -> str:
        return self.fixtures.text(case["expected"])


def check(
    manifest: dict[str, Any],
    kinds: set[str],
    run: Callable[[Case], bytes | str | None],
    expected: Callable[[Case], bytes | str],
) -> None:
    """Runs each case of `kinds`. A case the tool did not fail gives what
    `expected` reads; one it failed raises. A case `run` checks itself gives
    None. A kind with no cases fails: the expectations are missing, not
    met."""
    cases = [case for case in manifest["cases"] if case["kind"] in kinds]
    assert cases, f"compile-parity.json holds no {kinds} cases"
    found = []
    for case in cases:
        what = " ".join(case["tool"])
        try:
            got = run(case)
        except (divvun_hfst.HfstError, ValueError) as error:
            if case["ok"]:
                found.append(f"hfst {what}: raised {error!r}")
            continue
        if not case["ok"]:
            found.append(f"hfst {what}: the tool failed, and the package did not")
        elif got is not None and got != expected(case):
            found.append(f"hfst {what}: {got!r:.300}\n  tool: {expected(case)!r:.300}")
    assert not found, f"{len(found)} of {len(cases)} cases differ:\n" + "\n".join(found)


def stream(ts: list[Transducer]) -> bytes:
    return b"".join(t.to_bytes() for t in ts)


def lookup_text(reports: list[divvun_hfst.LookupReport]) -> str:
    """What `hfst lookup` prints for `reports`, in its Xerox format."""
    text = ""
    for report in reports:
        if not report.results:
            text += f"{report.input}\t{report.input}+?\tinf\n\n"
            continue
        for r in report.results:
            text += f"{report.input}\t{r.output}\t{r.weight:.6f}\n"
        text += f"{report.input}\t[...cyclic...]\n\n" if report.infinite else "\n"
    return text


def att_pieces(text: str) -> list[str]:
    """The AT&T text of each transducer, as the tool reads them one by one,
    each with the `--` line that ends it."""
    pieces = [""]
    for line in text.splitlines(keepends=True):
        pieces[-1] += line
        if line.strip() == "--":
            pieces.append("")
    return [piece for piece in pieces if piece]
