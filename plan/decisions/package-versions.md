---
id [dec:hfst:package-versions]
epitome "@divvun/hfst and divvun-hfst are versioned on their own, each in its binding crate, and released from their own tags: wasm/v<version> the npm package, python/v<version> the Python package. The library and the tools keep the workspace's version."
state @decided
category @existence
scope {
    elements ()
    rules (
        [spec:hfst:req:javascript.version]
        [spec:hfst:req:python.module]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-10T00:00:00Z"
edges {
    requires ([dec:hfst:python-package])
}
alternatives (
    {
        option "Publish both packages at the workspace's version, the library's."
        rejected_because "A fix to one binding would need a release of the library, and every release of the library would publish both packages again with nothing in them changed. A package's version would also say nothing about its own surface: a breaking change to the Python API could ride a patch release of the library."
    }
    {
        option "Keep each package's version in its manifest alone, js/hfst/package.json and python/pyproject.toml."
        rejected_because "version() and divvun_hfst.__version__ are compiled from the binding crate, so the crate would carry a second copy that could drift. maturin reads the crate's version for the wheels, so the crate is the one place, and js/hfst/package.json, which npm needs, is checked against it."
    }
)
consequences {
    accepted (
        "crates/hfst-wasm and crates/hfst-python carry their package's version rather than the workspace's. version() and divvun_hfst.__version__ report it. A Rust test checks js/hfst/package.json against crates/hfst-wasm, and python/tests checks the installed package against crates/hfst-python."
        "A wasm/v<version> tag releases @divvun/hfst to npm, and a python/v<version> tag releases the divvun-hfst wheels and sdist to PyPI. Building them from those tags is CI's work."
        "Both packages start their own line at 0.1.0: @divvun/hfst is at 0.1.0 on npm already, and divvun-hfst is new."
    )
    deferred (
        "Reporting the library version a package was built from, beside the package's own."
        "The CI pipelines that build, test and publish each package from its tag."
    )
}
codifies ([spec:hfst:req:javascript.version] [spec:hfst:req:python.module])
establishes ()
---

## Rationale

The library, the npm package and the Python package change at different
rates. Most library changes fix the engine and leave both binding surfaces
alone, and most binding changes leave the engine alone. One version for all
three would make every release of one a release of the others.

Each binding crate is unpublished and exists only to be its package, so its
own version is the package's: maturin reads it for the wheels, the npm
package's manifest is checked against it, and both bindings report it at run
time.
