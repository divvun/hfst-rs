"""The package as it installs: its name, version, licence, wheel and types."""

from __future__ import annotations

import ast
import inspect
import os
import re
import subprocess
import sys
import tomllib
import typing
import zipfile
from importlib import metadata
from pathlib import Path

import pytest

import divvun_hfst
import divvun_hfst._divvun_hfst
from conftest import ROOT, Fixtures

PACKAGE = Path(divvun_hfst.__file__).parent
STUB = ast.parse((PACKAGE / "_divvun_hfst.pyi").read_text(encoding="utf-8"))
OPTIONS = ast.parse((PACKAGE / "_options.py").read_text(encoding="utf-8"))
LICENCES = {"COPYING": ROOT / "crates/hfst-cli/COPYING", "COPYING.LESSER": ROOT / "COPYING"}


def toml(path: Path) -> dict[str, typing.Any]:
    with path.open("rb") as f:
        return tomllib.load(f)


def stub_names() -> dict[str, ast.stmt]:
    names: dict[str, ast.stmt] = {}
    for node in STUB.body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef)):
            names[node.name] = node
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names[node.target.id] = node
    return names


def stub_class(name: str) -> ast.ClassDef:
    node = stub_names()[name]
    assert isinstance(node, ast.ClassDef)
    return node


def stub_methods(cls: str) -> dict[str, ast.FunctionDef]:
    return {
        node.name: node
        for node in stub_class(cls).body
        if isinstance(node, ast.FunctionDef)
        and not any(isinstance(d, ast.Name) and d.id == "property" for d in node.decorator_list)
    }


def properties(cls: str) -> dict[str, str]:
    """Each read-only attribute of `cls` in the stub, a property or a
    `Final` field, with its type."""
    found: dict[str, str] = {}
    for node in stub_class(cls).body:
        if isinstance(node, ast.FunctionDef) and node.returns is not None:
            if any(isinstance(d, ast.Name) and d.id == "property" for d in node.decorator_list):
                found[node.name] = ast.unparse(node.returns)
        elif (
            isinstance(node, ast.AnnAssign)
            and isinstance(node.target, ast.Name)
            and isinstance(node.annotation, ast.Subscript)
            and ast.unparse(node.annotation.value) == "Final"
        ):
            found[node.target.id] = ast.unparse(node.annotation.slice)
    return found


def stub_signature(node: ast.FunctionDef) -> str:
    """The signature the runtime gives a function the stub declares so."""
    args = node.args
    parts: list[str] = []
    positional = [*args.posonlyargs, *args.args]
    defaults = [None] * (len(positional) - len(args.defaults)) + list(args.defaults)
    for i, (arg, default) in enumerate(zip(positional, defaults)):
        parts.append(arg.arg if default is None else f"{arg.arg}={ast.unparse(default)}")
        if args.posonlyargs and i == len(args.posonlyargs) - 1:
            parts.append("/")
    if args.kwonlyargs:
        parts.append("*")
        for arg, kw_default in zip(args.kwonlyargs, args.kw_defaults):
            parts.append(arg.arg if kw_default is None else f"{arg.arg}={ast.unparse(kw_default)}")
    return f"({', '.join(parts)})"


# [spec:hfst:req:python.module/test]
def test_the_version_is_the_binding_crates() -> None:
    version = toml(ROOT / "crates/hfst-python/Cargo.toml")["package"]["version"]
    assert isinstance(version, str)
    assert divvun_hfst.__version__ == version
    assert metadata.version("divvun-hfst") == version


@pytest.fixture(scope="module")
def wheel(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """The wheel `DIVVUN_HFST_WHEEL` names, which CI sets to the one it tests
    and publishes, or else one maturin builds from python/ as a release
    would."""
    given = os.environ.get("DIVVUN_HFST_WHEEL")
    if given:
        named = Path(given)
        assert named.is_file(), f"DIVVUN_HFST_WHEEL names {given}, not a file from {Path.cwd()}"
        return named
    out = tmp_path_factory.mktemp("wheel")
    subprocess.run(
        [sys.executable, "-m", "maturin", "build", "--quiet", "--out", out],
        cwd=ROOT / "python",
        check=True,
    )
    [built] = out.glob("*.whl")
    return built


# [spec:hfst:req:python.module/test]
def test_the_wheel_is_divvun_hfst_under_lgpl(wheel: Path) -> None:
    meta = metadata.metadata("divvun-hfst")
    assert meta["Name"] == "divvun-hfst"
    assert meta["License-Expression"] == "LGPL-3.0-or-later"
    assert meta["Requires-Python"] == ">=3.11"
    version = divvun_hfst.__version__
    info = f"divvun_hfst-{version}.dist-info"
    with zipfile.ZipFile(wheel) as built:
        files = set(built.namelist())
        texts = {name: built.read(f"{info}/licenses/{name}") for name in LICENCES}
    package = {f for f in files if f.startswith("divvun_hfst/")}
    native = {f for f in package if re.fullmatch(r"divvun_hfst/_divvun_hfst\.(abi3\.so|pyd)", f)}
    assert len(native) == 1
    assert package - native == {
        "divvun_hfst/__init__.py",
        "divvun_hfst/_options.py",
        "divvun_hfst/_divvun_hfst.pyi",
        "divvun_hfst/py.typed",
    }
    assert {f.split("/")[0] for f in files} == {"divvun_hfst", info}
    for name, source in LICENCES.items():
        assert texts[name].decode().strip() == source.read_text().strip(), name
    assert b"LESSER" in texts["COPYING.LESSER"]


# [spec:hfst:req:python.module/test]
def test_a_named_wheel_is_the_one_imported() -> None:
    if not os.environ.get("DIVVUN_HFST_WHEEL"):
        pytest.skip("no DIVVUN_HFST_WHEEL: the tests run against the checkout's build")
    assert PACKAGE.resolve() != (ROOT / "python" / "divvun_hfst").resolve()
    assert Path(divvun_hfst._divvun_hfst.__file__ or "").parent == PACKAGE


# [spec:hfst:req:python.module/test]
def test_one_wheel_serves_cpython_311_on(wheel: Path) -> None:
    pyo3 = toml(ROOT / "crates/hfst-python/Cargo.toml")["dependencies"]["pyo3"]
    assert pyo3["features"] == ["abi3-py311"]
    assert f"-{divvun_hfst.__version__}-cp311-abi3-" in wheel.name
    with zipfile.ZipFile(wheel) as built:
        [native] = [
            f
            for f in built.namelist()
            if f.startswith("divvun_hfst/_divvun_hfst.") and not f.endswith(".pyi")
        ]
    assert native in ("divvun_hfst/_divvun_hfst.abi3.so", "divvun_hfst/_divvun_hfst.pyd")


# [spec:hfst:req:python.typed/test]
def test_the_stub_declares_the_whole_api() -> None:
    assert (PACKAGE / "py.typed").is_file()
    assert set(divvun_hfst.__all__) <= set(stub_names()) | {"TokeniseOptions", "FormatOptions"}
    declared = {n for n in stub_names() if not n.startswith("_")} | {"__version__"}
    native = {n for n in dir(divvun_hfst._divvun_hfst) if not n.startswith("_")} | {"__version__"}
    assert declared == native
    assert all(hasattr(divvun_hfst._divvun_hfst, n) for n in stub_names())


# [spec:hfst:req:python.typed/test]
def test_no_public_signature_uses_any() -> None:
    for node in [*ast.walk(STUB), *ast.walk(OPTIONS)]:
        assert not (isinstance(node, ast.Name) and node.id == "Any")
        assert not (isinstance(node, ast.Attribute) and node.attr == "Any")


# [spec:hfst:req:python.typed/test]
def test_stub_signatures_are_the_runtimes() -> None:
    for name, node in stub_names().items():
        if isinstance(node, ast.FunctionDef):
            runtime = getattr(divvun_hfst._divvun_hfst, name)
            assert str(inspect.signature(runtime)) == stub_signature(node), name
        elif isinstance(node, ast.ClassDef):
            runtime_class = getattr(divvun_hfst._divvun_hfst, name)
            for method, declared in stub_methods(name).items():
                found = getattr(runtime_class, method)
                assert str(inspect.signature(found)) == stub_signature(declared), method
            for attribute in properties(name):
                descriptor = inspect.getattr_static(runtime_class, attribute)
                assert type(descriptor).__name__ == "getset_descriptor", f"{name}.{attribute}"


# [spec:hfst:req:python.typed/test]
def test_the_literals_are_the_runtimes() -> None:
    for name in ("HfstErrorCode", "TransducerType", "TokeniseFormat"):
        node = stub_names()[name]
        assert isinstance(node, ast.AnnAssign) and node.value is not None
        declared = ast.literal_eval(ast.unparse(node.value).removeprefix("Literal"))
        assert tuple(declared) == typing.get_args(getattr(divvun_hfst, name)), name


# [spec:hfst:req:python.typed/test]
def test_results_carry_their_declared_types(fixtures: Fixtures) -> None:
    kinds: dict[str, type | tuple[type, ...]] = {
        "int": int,
        "bool": bool,
        "str": str,
        "float": float,
        "str | None": (str, type(None)),
        "tuple[str, ...]": tuple,
        "tuple[Reading, ...]": tuple,
        "dict[str, str]": dict,
        "AcceptorInfo": divvun_hfst.AcceptorInfo,
        "TransducerType": str,
        "Literal['acceptor']": str,
        "Literal['error-model']": str,
    }

    def check(cls: str, value: object) -> None:
        for name, annotation in properties(cls).items():
            got = getattr(value, name)
            assert isinstance(got, kinds[annotation]), f"{cls}.{name}"
            assert annotation != "int" or not isinstance(got, bool), f"{cls}.{name}"

    transducer = divvun_hfst.load_transducer(fixtures.path("first.hfst"))
    check("Transducer", transducer)
    check("LookupResult", transducer.lookup("cat")[0])
    check("OptimisedTransducer", divvun_hfst.load_optimised_transducer(fixtures.path("first.hfst")))
    [token] = divvun_hfst.load_tokeniser(fixtures.path("tokenise.pmhfst")).tokenise("cat")
    check("Token", token)
    check("Reading", token.readings[0])
    acceptor = divvun_hfst.load_dhfst(fixtures.path("speller.acceptor.dhfst"))
    check("DhfstAcceptor", acceptor)
    assert isinstance(acceptor, divvun_hfst.DhfstAcceptor)
    check("AcceptorInfo", acceptor.info)
    check("DhfstErrorModel", divvun_hfst.load_dhfst(fixtures.path("errmodel.error-model.dhfst")))


# [spec:hfst:req:python.typed/test]
def test_names_follow_python_conventions() -> None:
    snake = re.compile(r"^_?_?[a-z][a-z0-9_]*$")
    pascal = re.compile(r"^[A-Z][A-Za-z0-9]*$")
    for node in [*ast.walk(STUB), *ast.walk(OPTIONS)]:
        if isinstance(node, ast.ClassDef):
            assert pascal.match(node.name), node.name
            for field in node.body:
                if isinstance(field, ast.AnnAssign) and isinstance(field.target, ast.Name):
                    assert snake.match(field.target.id), field.target.id
        elif isinstance(node, ast.FunctionDef):
            assert snake.match(node.name), node.name
            for arg in [*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs]:
                assert snake.match(arg.arg), arg.arg
