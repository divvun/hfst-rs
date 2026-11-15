#!/bin/sh
# The Swift platform's check, as .config/nplan/config.styx runs it. A check's
# command is not run by a shell, so its steps live here: the linking test's
# second library's own test, the host's slice of the XCFramework, the
# fixtures the suite reads, and the suite on macOS.
set -eu
cd "$(dirname "$0")/.."
cargo test --quiet --locked --manifest-path swift/link-probe/Cargo.toml --target-dir target/swift/link-probe
swift/build.sh --host
node js/test/fixtures.mjs target/swift/fixtures >/dev/null
cd swift
DIVVUN_HFST_FIXTURES="$PWD/../target/swift/fixtures" swift test
