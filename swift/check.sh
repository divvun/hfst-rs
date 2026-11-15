#!/bin/sh
# The Swift platform's check, as .config/nplan/config.styx runs it. A check's
# command is not run by a shell, so its steps live here: the host's slice of
# the XCFramework, the fixtures the suite reads, and the suite on macOS.
set -eu
cd "$(dirname "$0")/.."
swift/build.sh --host
node js/test/fixtures.mjs target/swift/fixtures >/dev/null
cd swift
DIVVUN_HFST_FIXTURES="$PWD/../target/swift/fixtures" swift test
