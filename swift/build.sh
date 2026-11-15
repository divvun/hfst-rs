#!/bin/sh
# [spec:hfst:req:swift.build]
# Builds divvun.hfst, the Swift package: the static library crates/hfst-swift
# makes for each platform, the Swift and the headers UniFFI generates for it,
# and swift/HFSTFFI.xcframework, which swift/Package.swift names.
#
#   swift/build.sh            every slice: macOS, iOS and the iOS simulator
#   swift/build.sh --host     the host's macOS slice alone, for development
#                             and `swift test`
#   swift/build.sh --archive  every slice, then dist/swift/hfst-<version>.zip
#                             and dist/swift/package-metadata.json, what
#                             pahkat publishes
#
# Needs Xcode and, for every slice, the Rust targets aarch64-apple-darwin,
# x86_64-apple-darwin, aarch64-apple-ios and aarch64-apple-ios-sim.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
cd "$root"

host=false
archive=false
for arg in "$@"; do
    case "$arg" in
    --host) host=true ;;
    --archive) archive=true ;;
    *)
        echo "swift/build.sh takes --host or --archive, not $arg" >&2
        exit 2
        ;;
    esac
done
if $host && $archive; then
    echo "an archive holds every slice, so --host and --archive do not go together" >&2
    exit 2
fi

target="${CARGO_TARGET_DIR:-$root/target}"
out="$target/swift"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' crates/hfst-swift-runtime/Cargo.toml | head -n 1)"

# [spec:hfst:req:swift.platforms]
# Each slice is built for the package's minimum, iOS 13 and macOS 11, unless
# the Rust target's own minimum is higher: the arm64 simulator's is iOS 14.
deployment() {
    case "$1" in
    *-apple-darwin) echo "MACOSX_DEPLOYMENT_TARGET=11.0" ;;
    aarch64-apple-ios) echo "IPHONEOS_DEPLOYMENT_TARGET=13.0" ;;
    aarch64-apple-ios-sim) echo "IPHONEOS_DEPLOYMENT_TARGET=14.0" ;;
    esac
}

if $host; then
    case "$(uname -m)" in
    arm64) targets="aarch64-apple-darwin" ;;
    *) targets="x86_64-apple-darwin" ;;
    esac
else
    targets="aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim"
fi

for t in $targets; do
    echo "--- libhfst_swift.a for $t"
    env "$(deployment "$t")" cargo build --release --locked -p hfst-swift --lib --target "$t"
done
library() {
    echo "$target/$1/release/libhfst_swift.a"
}

echo "--- the Swift, headers and module map UniFFI generates"
generated="$out/generated"
rm -rf "$generated"
first="$(echo "$targets" | cut -d ' ' -f 1)"
cargo run --release --locked -p hfst-swift --example uniffi-bindgen-swift -- \
    "$(library "$first")" "$generated" \
    --swift-sources --headers --modulemap \
    --module-name HFSTFFI --modulemap-filename module.modulemap
for source in "$generated"/*.swift; do
    module="$(basename "$source" .swift)"
    mkdir -p "$here/Sources/$module"
    cp "$source" "$here/Sources/$module/"
done

# [spec:hfst:req:swift.linking]
# The headers and the module map go in Headers/HFSTFFI/ rather than at the top
# of Headers: Xcode copies every static XCFramework's headers into one
# directory, where another package's module.modulemap would meet this one.
include="$out/include"
rm -rf "$include"
mkdir -p "$include/HFSTFFI"
cp "$generated"/*.h "$generated/module.modulemap" "$include/HFSTFFI/"

echo "--- HFSTFFI.xcframework"
set --
if $host; then
    set -- -library "$(library "$first")" -headers "$include"
else
    mkdir -p "$out/macos"
    lipo -create -output "$out/macos/libhfst_swift.a" \
        "$(library aarch64-apple-darwin)" "$(library x86_64-apple-darwin)"
    set -- \
        -library "$out/macos/libhfst_swift.a" -headers "$include" \
        -library "$(library aarch64-apple-ios)" -headers "$include" \
        -library "$(library aarch64-apple-ios-sim)" -headers "$include"
fi
rm -rf "$here/HFSTFFI.xcframework"
xcodebuild -create-xcframework "$@" -output "$here/HFSTFFI.xcframework"

if ! $archive; then
    exit 0
fi

# [spec:hfst:req:swift.registry]
# The archive holds what the package builds and nothing else: its manifest,
# the Swift of its four targets and the XCFramework. They are copied to a
# directory of their own first, since `swift package archive-source` zips the
# whole package directory, with its tests, this script and .build.
echo "--- dist/swift/hfst-$version.zip"
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
package="$staging/hfst"
mkdir -p "$package/Sources"
cp "$here/Package.swift" "$package/"
for module in HFST HFSTCompile HFSTGlue HFSTCompileGlue; do
    if [ -d "$here/Sources/$module" ]; then
        cp -R "$here/Sources/$module" "$package/Sources/"
    fi
done
cp -R "$here/HFSTFFI.xcframework" "$package/"
dist="$root/dist/swift"
rm -rf "$dist"
mkdir -p "$dist"
(cd "$package" && swift package archive-source --output "$dist/hfst-$version.zip")

cat >"$dist/package-metadata.json" <<EOF
{
  "author": { "name": "UiT The Arctic University of Norway" },
  "description": "Divvun HFST for Swift: reads optimized-lookup transducers, pmatch tokenisers and DHFST spellers and runs lookup, tokenisation and DHFST acceptor lookup, and its HFSTCompile product compiles, transforms, inspects and writes transducers as the hfst tools do. macOS, iOS and the iOS simulator.",
  "licenseURL": "https://github.com/divvun/hfst-rs/blob/main/COPYING",
  "readmeURL": "https://github.com/divvun/hfst-rs/blob/main/swift/README.md",
  "repositoryURLs": ["https://github.com/divvun/hfst-rs", "https://github.com/divvun/hfst-rs.git", "git@github.com:divvun/hfst-rs.git"]
}
EOF
ls -l "$dist"
