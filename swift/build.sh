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

# What `ld -r -platform_version` calls the platform of a target, and its
# minimum.
platform() {
    case "$1" in
    *-apple-darwin) echo "macos 11.0" ;;
    aarch64-apple-ios) echo "ios 13.0" ;;
    aarch64-apple-ios-sim) echo "ios-simulator 14.0" ;;
    esac
}

architecture() {
    case "$1" in
    aarch64-*) echo arm64 ;;
    x86_64-*) echo x86_64 ;;
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
    env "$(deployment "$t")" cargo build --profile swift-release --locked -p hfst-swift --lib --target "$t"
done
library() {
    echo "$target/$1/swift-release/libhfst_swift.a"
}

# UniFFI reads its metadata from the static library's own symbols, which the
# prelink makes local, so the bindings come from the library cargo built.
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
# The compile glue uses the runtime glue's error, diagnostics, lookup results
# and tokeniser. UniFFI writes them as if both were one Swift module, so the
# compile glue's module imports the runtime glue's.
perl -0pi -e 's/^import Foundation\n/import Foundation\nimport HFSTGlue\n/m' \
    "$here/Sources/HFSTCompileGlue/HFSTCompileGlue.swift"

# [spec:hfst:req:swift.linking+1]
# Prelinks the static library for target $3 ($1) into one object and
# archives it as $2. Every Rust static library holds the standard library and
# its own copies of the crates it shares with others; made local, they cannot
# meet another library's, so this one links beside any other, LTO'd or not.
# A second pass makes the UniFFI C interface private external too: Swift in
# the same image still reaches it, but a dynamic framework an app wraps the
# package in does not export it, and so keeps only what it calls.
#
# The object keeps MH_SUBSECTIONS_VIA_SYMBOLS, which lets an app's linker drop
# each function the app does not reach, the compilers among them, only if
# every input has it. compiler-rt's outline atomics helpers lack it, though
# each defines one function and nothing else, which is all the flag
# promises, so they are given it. A member without it that defines more
# stays beside the object, its symbols private external as compiler-rt makes
# them.
prelink() {
    work="$(mktemp -d)"
    mkdir "$work/members" "$work/beside"
    (cd "$work/members" && ar x "$1")
    rm -f "$work/members"/__.SYMDEF*
    members="$(ar t "$1" | grep -vc '^__.SYMDEF')"
    if [ "$(ls "$work/members" | wc -l)" -ne "$members" ]; then
        echo "two members of $1 share a name, so ar x lost one" >&2
        exit 1
    fi
    for object in "$work/members"/*; do
        if otool -hv "$object" | tail -n 1 | grep -q SUBSECTIONS_VIA_SYMBOLS; then
            continue
        fi
        if [ "$(nm --no-llvm-bc -gU "$object" | grep -c ' T ')" -gt 1 ]; then
            mv "$object" "$work/beside/"
            continue
        fi
        perl -e 'open(F, "+<", $ARGV[0]) or die; binmode F; seek(F, 24, 0);
            read(F, $b, 4); seek(F, 24, 0); print F pack("V", unpack("V", $b) | 0x2000);' "$object"
    done
    printf '_uniffi_hfst_swift_*\n_ffi_hfst_swift_*\n' >"$work/exports"
    arch="$(architecture "$3")"
    # shellcheck disable=SC2046
    set -- "$1" "$2" "$3" $(platform "$3")
    xcrun ld -r -arch "$arch" -platform_version "$4" "$5" "$5" \
        -exported_symbols_list "$work/exports" -o "$work/local.o" "$work/members"/*
    xcrun ld -r -arch "$arch" -platform_version "$4" "$5" "$5" -keep_private_externs \
        -unexported_symbols_list "$work/exports" -o "$work/prelinked.o" "$work/local.o"
    if ! otool -hv "$work/prelinked.o" | tail -n 1 | grep -q SUBSECTIONS_VIA_SYMBOLS; then
        echo "the prelinked library for $3 lost MH_SUBSECTIONS_VIA_SYMBOLS" >&2
        exit 1
    fi
    if nm --no-llvm-bc -m "$work/prelinked.o" | grep -v '(undefined)' | grep -E '\) (weak )?external ' | grep -q .; then
        echo "the prelinked library for $3 exports a symbol" >&2
        exit 1
    fi
    if nm --no-llvm-bc -m "$work/prelinked.o" | grep -v '(undefined)' | grep -E '\) (weak )?private external ' | grep -v ' _\(uniffi\|ffi\)_hfst_swift_' | grep -q .; then
        echo "the prelinked library for $3 keeps a symbol besides the C interface" >&2
        exit 1
    fi
    mkdir -p "$(dirname "$2")"
    rm -f "$2"
    libtool -static -o "$2" "$work/prelinked.o" "$work/beside"/* 2>/dev/null
    rm -rf "$work"
}
prelinked() {
    echo "$out/prelinked/$1/libhfst_swift.a"
}
for t in $targets; do
    echo "--- prelinking libhfst_swift.a for $t"
    prelink "$(library "$t")" "$(prelinked "$t")" "$t"
done

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
    set -- -library "$(prelinked "$first")" -headers "$include"
else
    mkdir -p "$out/macos"
    lipo -create -output "$out/macos/libhfst_swift.a" \
        "$(prelinked aarch64-apple-darwin)" "$(prelinked x86_64-apple-darwin)"
    set -- \
        -library "$out/macos/libhfst_swift.a" -headers "$include" \
        -library "$(prelinked aarch64-apple-ios)" -headers "$include" \
        -library "$(prelinked aarch64-apple-ios-sim)" -headers "$include"
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
