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

# [spec:hfst:req:swift.linking+1/test]
# The linking test's second Rust library, swift/link-probe, built on its own
# as its Cargo.toml says: with LTO and without the prelink, so std's symbols
# stay global in it, as in another package's library. It goes in
# swift/LinkProbeFFI.xcframework, and its Swift in swift/Tests/LinkProbe.
# UniFFI's bindgen reads the probe's uniffi.toml from the directory it runs
# in.
echo "--- the linking test's second library"
probe="$out/link-probe"
for t in $targets; do
    env "$(deployment "$t")" cargo build --release --locked \
        --manifest-path "$here/link-probe/Cargo.toml" --target-dir "$probe" --target "$t"
done
probed() {
    echo "$probe/$1/release/libhfst_link_probe.a"
}
rm -rf "$probe/generated" "$probe/include"
(cd "$here/link-probe" && "$target/release/examples/uniffi-bindgen-swift" \
    "$(probed "$first")" "$probe/generated" \
    --swift-sources --headers --modulemap \
    --module-name LinkProbeFFI --modulemap-filename module.modulemap)
mkdir -p "$here/Tests/LinkProbe" "$probe/include/LinkProbeFFI"
cp "$probe/generated/LinkProbe.swift" "$here/Tests/LinkProbe/"
cp "$probe/generated"/*.h "$probe/generated/module.modulemap" "$probe/include/LinkProbeFFI/"
set --
if $host; then
    set -- -library "$(probed "$first")" -headers "$probe/include"
else
    mkdir -p "$probe/macos"
    lipo -create -output "$probe/macos/libhfst_link_probe.a" \
        "$(probed aarch64-apple-darwin)" "$(probed x86_64-apple-darwin)"
    set -- \
        -library "$probe/macos/libhfst_link_probe.a" -headers "$probe/include" \
        -library "$(probed aarch64-apple-ios)" -headers "$probe/include" \
        -library "$(probed aarch64-apple-ios-sim)" -headers "$probe/include"
fi
rm -rf "$here/LinkProbeFFI.xcframework"
xcodebuild -create-xcframework "$@" -output "$here/LinkProbeFFI.xcframework"

if ! $archive; then
    exit 0
fi

# [spec:hfst:req:swift.registry+1]
# The archive holds what the package builds, its README and the licence texts
# divvun-hfst carries, and nothing else: its manifest, the Swift of its four
# targets, the XCFramework, README.md, COPYING and COPYING.LESSER. They are
# copied to a directory of their own first, since `swift package
# archive-source` zips the whole package directory, with its tests, this
# script and .build.
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
cp "$here/README.md" "$package/README.md"
cp -L "$root/python/COPYING" "$package/COPYING"
cp -L "$root/python/COPYING.LESSER" "$package/COPYING.LESSER"
dist="$root/dist/swift"
rm -rf "$dist"
mkdir -p "$dist"
(cd "$package" && swift package archive-source --output "$dist/hfst-$version.zip")
for file in Package.swift README.md COPYING COPYING.LESSER; do
    if ! unzip -Z1 "$dist/hfst-$version.zip" | grep -qx "hfst/$file"; then
        echo "dist/swift/hfst-$version.zip lacks $file" >&2
        exit 1
    fi
done

cat >"$dist/package-metadata.json" <<EOF
{
  "author": { "name": "UiT The Arctic University of Norway", "url": "https://github.com/divvun/hfst-rs" },
  "description": "Divvun HFST for Swift: reads optimized-lookup transducers, pmatch tokenisers and DHFST spellers and runs lookup, tokenisation and DHFST acceptor lookup, and its HFSTCompile product compiles, transforms, inspects and writes transducers as the hfst tools do. macOS, iOS and the iOS simulator.",
  "licenseURL": "https://github.com/divvun/hfst-rs/blob/swift/v$version/COPYING",
  "readmeURL": "https://github.com/divvun/hfst-rs/blob/swift/v$version/swift/README.md",
  "repositoryURLs": ["https://github.com/divvun/hfst-rs", "https://github.com/divvun/hfst-rs.git", "git@github.com:divvun/hfst-rs.git"]
}
EOF
ls -l "$dist"

# Whether the binary $1 holds the compilers' code: a function whose own
# path, as nm demangles it, is in a compiler crate, the pmatch compiler or the
# compile glue. A generic function a compiler instantiated, or one LTO found
# identical to one of HFST's and merged, can bear a compiler's name in its
# mangling while being code HFST needs, so a function counts only where its
# path begins.
holds_compilers() {
    nm --no-llvm-bc -C "$1" | awk '$2 ~ /^[tT]$/' | cut -d ' ' -f 3- |
        grep -qE '^<?(nfst_(xre|lexc|twolc|xfst|pmatch)|hfst::pmatch_compiler|hfst_swift_compile)::'
}
checks="$staging/checks"
mkdir -p "$checks"
megabytes() {
    wc -c <"$1" | awk '{ printf "%.2f MB", $1 / 1000000 }'
}
stripped() {
    strip -x -o "$checks/$2.stripped" "$1"
    megabytes "$checks/$2.stripped"
}

# [spec:hfst:req:swift.runtime-product/test]
# Links a macOS program that takes the address of every C function the
# headers $2... declare, so the linker keeps them and all they reach and
# drops the rest, and names it $1.
keeping() {
    name="$1"
    shift
    functions="$(cat "$@" | grep -oE '(uniffi|ffi)_hfst_swift_[A-Za-z0-9_]+[[:space:]]*\(' | tr -d ' (' | sort -u)"
    {
        for f in $functions; do echo "extern void $f(void);"; done
        echo "void *const kept[] = {"
        for f in $functions; do echo "    (void *)$f,"; done
        echo "};"
        echo "int main(void) {"
        echo "    volatile int n = 0;"
        echo "    for (unsigned long i = 0; i < sizeof kept / sizeof *kept; i++) n += kept[i] != 0;"
        echo "    return n == 0;"
        echo "}"
    } >"$checks/$name.c"
    xcrun clang -arch arm64 -mmacosx-version-min=11.0 -w "$checks/$name.c" \
        "$(prelinked aarch64-apple-darwin)" -Wl,-dead_strip -o "$checks/$name"
}
echo "--- what each product's glue reaches"
keeping hfst-glue "$include/HFSTFFI/HFSTGlueFFI.h"
keeping both-glues "$include/HFSTFFI/HFSTGlueFFI.h" "$include/HFSTFFI/HFSTCompileGlueFFI.h"
if holds_compilers "$checks/hfst-glue"; then
    echo "a program that calls all of HFST's glue holds the compilers" >&2
    exit 1
fi
if ! holds_compilers "$checks/both-glues"; then
    echo "a program that calls HFSTCompile's glue holds no compiler code the check can see" >&2
    exit 1
fi
echo "macOS arm64 program keeping every C function, stripped: of HFST's glue $(stripped "$checks/hfst-glue" hfst-glue), of both products' $(stripped "$checks/both-glues" both-glues)"

# [spec:hfst:req:swift.runtime-product/test]
# [spec:hfst:req:swift.linking+1/test]
# Builds from the unpacked archive, as an app depends on it: two macOS
# programs, one using only HFST and one HFSTCompile, which are run; and two
# iOS device frameworks in release, one using all of HFST and one HFSTCompile
# too. A framework must not export the C interface, which would keep all of
# it, and the one using HFST alone must hold none of the compilers.
echo "--- an app of the archive"
consumer="$checks/consumer"
mkdir -p "$consumer/unpacked"
unzip -q "$dist/hfst-$version.zip" -d "$consumer/unpacked"
app="$consumer/app"
mkdir -p "$app/Sources/Look" "$app/Sources/Compile" "$app/Sources/Looks" "$app/Sources/Uses"
cat >"$app/Package.swift" <<'EOF'
// swift-tools-version:6.0
import PackageDescription

let package = Package(
    name: "Consumer",
    platforms: [.iOS(.v13), .macOS(.v11)],
    products: [
        .library(name: "Looks", type: .dynamic, targets: ["Looks"]),
        .library(name: "Uses", type: .dynamic, targets: ["Uses"]),
    ],
    dependencies: [.package(path: "../unpacked/hfst")],
    targets: [
        .executableTarget(name: "Look", dependencies: [.product(name: "HFST", package: "hfst")]),
        .executableTarget(name: "Compile", dependencies: [.product(name: "HFSTCompile", package: "hfst")]),
        .target(name: "Looks", dependencies: [.product(name: "HFST", package: "hfst")]),
        .target(name: "Uses", dependencies: [.product(name: "HFSTCompile", package: "hfst")]),
    ]
)
EOF
cat >"$app/Sources/Compile/main.swift" <<'EOF'
import Foundation
import HFSTCompile

let words = try regex("{cat}:{dog}").value
try words.convert(to: .optimizedLookupWeighted)
try words.data().write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
print(try words.lookup("cat").map(\.output).joined(separator: " "))
EOF
cat >"$app/Sources/Look/main.swift" <<'EOF'
import Foundation
import HFST

let words = try Transducer(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
print(try words.lookup("cat").map(\.output).joined(separator: " "))
EOF
cat >"$app/Sources/Looks/Looks.swift" <<'EOF'
import Foundation
import HFST

public func analyses(_ transducer: Data, _ word: String) throws -> [String] {
    let found = try Transducer.all(in: transducer).flatMap { try $0.lookup(word) }
    return try found.map(\.output) + OptimisedTransducer(data: transducer).lookup(word).map(\.output)
}

public func tokens(_ tokeniser: Data, _ text: String) throws -> [String] {
    let read = try Tokeniser(data: tokeniser)
    return try read.tokenise(text).map(\.surface) + [read.format(text, as: .cg)]
}

public func weight(_ speller: Data, _ word: String) throws -> Float? {
    switch try Dhfst(data: speller) {
    case let .acceptor(acceptor): acceptor.lookup(word)
    case let .errorModel(model): Float(model.stateCount)
    }
}

public func read(_ url: URL, _ word: String) throws -> Int {
    let transducer = try Transducer(contentsOf: url)
    let fast = try OptimisedTransducer(contentsOf: url)
    let tokeniser = try Tokeniser(contentsOf: url)
    guard case let .acceptor(speller) = try Dhfst(contentsOf: url) else { return 0 }
    return try transducer.lookup(word).count + fast.lookup(word).count
        + tokeniser.tokenise(word).count + (speller.lookup(word) == nil ? 0 : 1)
}

public func log(at level: HFSTLogLevel?) {
    HFSTLogging.level = level
    HFSTLogging.setHandler { print($0.message) }
}
EOF
cat >"$app/Sources/Uses/Uses.swift" <<'EOF'
import Foundation
import HFSTCompile

public func analyses(_ transducer: Data, _ word: String) throws -> [String] {
    let found = try Transducer.all(in: transducer).flatMap { try $0.lookup(word) }
    return try found.map(\.output) + OptimisedTransducer(data: transducer).lookup(word).map(\.output)
}

public func tokens(_ tokeniser: Data, _ text: String) throws -> [String] {
    let read = try Tokeniser(data: tokeniser)
    return try read.tokenise(text).map(\.surface) + [read.format(text, as: .cg)]
}

public func compile(_ sources: [String: String]) throws -> [Data] {
    var made: [Data] = []
    if let text = sources["regex"] { made.append(try regex(text).value.data()) }
    if let text = sources["lexc"] { made.append(try compileLexc([text]).value.data()) }
    if let text = sources["twolc"] { made += try compileTwolc(text).value.map { try $0.data() } }
    if let text = sources["xfst"] { made += try compileXfst(text).written.values }
    if let text = sources["pmatch"] { made.append(try compilePmatch(text).value) }
    if let text = sources["strings"] { made.append(try compileStrings(text).value.data()) }
    if let text = sources["att"] { made.append(try parseAtt(text).value.data()) }
    return made
}

public func change(_ data: Data, with other: Data) throws -> String {
    let changed = try Transducer(data: data)
    try changed.compose(Transducer(data: other))
    try changed.minimize()
    try changed.convert(to: .optimizedLookupWeighted)
    return try changed.attText() + changed.summarize().name
}
EOF
(cd "$app" && swift build -c release --scratch-path "$checks/build" --product Compile -q)
(cd "$app" && swift build -c release --scratch-path "$checks/build" --product Look -q)
bin="$(cd "$app" && swift build -c release --scratch-path "$checks/build" --show-bin-path)"
for program in Compile Look; do
    ran="$("$bin/$program" "$checks/words.hfst")"
    if [ "$ran" != dog ]; then
        echo "$program, built from the archive, printed $ran" >&2
        exit 1
    fi
done
echo "macOS programs from the archive, stripped: using HFST $(stripped "$bin/Look" Look), using HFSTCompile $(stripped "$bin/Compile" Compile)"

framework() {
    (cd "$app" && xcodebuild build -quiet -scheme "$1" -destination generic/platform=iOS \
        -configuration Release -derivedDataPath "$checks/derived" CODE_SIGNING_ALLOWED=NO)
    built="$checks/derived/Build/Products/Release-iphoneos/PackageFrameworks/$1.framework/$1"
    if nm -gU --no-llvm-bc "$built" | grep -qE '_(uniffi|ffi)_hfst_swift_'; then
        echo "the $1 framework exports the C interface" >&2
        exit 1
    fi
}
framework Looks
framework Uses
looks="$checks/derived/Build/Products/Release-iphoneos/PackageFrameworks/Looks.framework/Looks"
uses="$checks/derived/Build/Products/Release-iphoneos/PackageFrameworks/Uses.framework/Uses"
if holds_compilers "$looks"; then
    echo "an iOS framework that uses only HFST holds the compilers" >&2
    exit 1
fi
if ! holds_compilers "$uses"; then
    echo "an iOS framework that uses HFSTCompile holds no compiler code the check can see" >&2
    exit 1
fi
echo "iOS device frameworks from the archive, stripped: using HFST $(stripped "$looks" Looks), using HFSTCompile $(stripped "$uses" Uses)"
echo "the archive: $(megabytes "$dist/hfst-$version.zip"); HFSTFFI.xcframework: $(du -sk "$here/HFSTFFI.xcframework" | awk '{ printf "%.1f MB", $1 * 1024 / 1000000 }')"
