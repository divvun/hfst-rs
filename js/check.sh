#!/bin/sh
# The TypeScript platform's check, as .config/nplan/config.styx runs it:
# deno check of the package and its tests. A check's command is not run by a
# shell, so its step lives here.
set -eu
cd "$(dirname "$0")"
deno check
