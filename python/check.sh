#!/bin/sh
# The Python platform's checks, as .config/nplan/config.styx runs them:
#
#   python/check.sh test        the suite, against the extension maturin
#                               builds into python/.venv
#   python/check.sh typecheck   mypy in its own uvx environment: a uv sync
#                               there would uninstall the editable build
#
# A check's command is not run by a shell, so its steps live here.
set -eu
cd "$(dirname "$0")"
case "${1:-}" in
test)
    uv sync --quiet --no-install-project
    uv run --no-sync maturin develop --uv --quiet
    uv run --no-sync pytest -q
    ;;
typecheck)
    uvx --quiet --with 'pytest>=8' --from 'mypy>=1.13' mypy
    ;;
*)
    echo "usage: python/check.sh test|typecheck" >&2
    exit 2
    ;;
esac
