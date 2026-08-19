#!/bin/sh

set -eu

usage() {
    cat <<'EOF'
usage: profile-language-build.sh LANGUAGE REPOSITORY [-- COMMAND ...]

LANGUAGE is sma, sme, gle, or kal. REPOSITORY must already be configured for
the HFST backend being measured. COMMAND defaults to `make -j1`.

Environment:
  HFST_PROFILE_HFST_BIN       release hfst multiplexer to exercise
  HFST_PROFILE_OUTPUT         directory for metrics and invocation logs
  HFST_PROFILE_BASELINE_DIR   optional mirror tree compared with hfst compare
  HFST_RUN_KAL_PERF_GATE=1    required to permit the long KAL stress gate
EOF
}

if [ "$#" -lt 2 ]; then
    usage >&2
    exit 2
fi

language=$1
language_repo=$2
shift 2
if [ "${1-}" = "--" ]; then
    shift
fi
if [ "$#" -eq 0 ]; then
    set -- make -j1
fi

case $language in
    sma | sme | gle) ;;
    kal)
        if [ "${HFST_RUN_KAL_PERF_GATE-}" != 1 ]; then
            printf '%s\n' \
                'KAL is a final stress gate; set HFST_RUN_KAL_PERF_GATE=1 to run it.' >&2
            exit 2
        fi
        ;;
    *)
        printf 'unsupported language: %s\n' "$language" >&2
        usage >&2
        exit 2
        ;;
esac

if [ ! -d "$language_repo" ]; then
    printf 'language repository does not exist: %s\n' "$language_repo" >&2
    exit 2
fi

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd)
hfst_bin=${HFST_PROFILE_HFST_BIN:-$repo_root/target/release/hfst}
if [ ! -x "$hfst_bin" ]; then
    printf 'HFST executable does not exist: %s\n' "$hfst_bin" >&2
    printf '%s\n' 'build it with: cargo build --release -p hfst-cli' >&2
    exit 2
fi

stamp=$(date -u +%Y%m%dT%H%M%S)
output=${HFST_PROFILE_OUTPUT:-$repo_root/target/perf/$language-$stamp}
actual_dir=$output/actual
wrapper_dir=$output/bin
log_dir=$output/invocations
mkdir -p "$actual_dir" "$wrapper_dir" "$log_dir"

ln -s "$hfst_bin" "$actual_dir/hfst"
"$hfst_bin" install-symlinks "$actual_dir"
for actual_tool in "$actual_dir"/hfst-*; do
    tool=$(basename "$actual_tool")
    ln -s "$script_dir/hfst-profile-wrapper.sh" "$wrapper_dir/$tool"
done

{
    printf 'language\t%s\n' "$language"
    printf 'repository\t%s\n' "$(CDPATH= cd "$language_repo" && pwd)"
    printf 'hfst\t%s\n' "$hfst_bin"
    printf 'started\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    for argument in "$@"; do
        printf 'command_argument\t%s\n' "$argument"
    done
} >"$output/run.meta"

platform=$(uname -s)
set +e
case $platform in
    Darwin)
        (
            cd "$language_repo" || exit 2
            /usr/bin/time -l -o "$output/total.time" \
                env PATH="$wrapper_dir:$PATH" \
                HFST_PROFILE_ACTUAL_DIR="$actual_dir" \
                HFST_PROFILE_LOG_DIR="$log_dir" \
                "$@"
        )
        status=$?
        awk '
            / real .* user .* sys/ {
                print "wall_seconds\t" $1
                print "user_seconds\t" $3
                print "system_seconds\t" $5
            }
            /maximum resident set size/ {
                print "max_rss_bytes\t" $1
            }
        ' "$output/total.time" >"$output/total.metrics"
        ;;
    Linux)
        (
            cd "$language_repo" || exit 2
            /usr/bin/time -f 'wall_seconds\t%e\nuser_seconds\t%U\nsystem_seconds\t%S\nmax_rss_kib\t%M' \
                -o "$output/total.time" \
                env PATH="$wrapper_dir:$PATH" \
                HFST_PROFILE_ACTUAL_DIR="$actual_dir" \
                HFST_PROFILE_LOG_DIR="$log_dir" \
                "$@"
        )
        status=$?
        awk -F '\t' '
            { print }
            $1 == "max_rss_kib" {
                printf "max_rss_bytes\t%.0f\n", $2 * 1024
            }
        ' "$output/total.time" >"$output/total.metrics"
        ;;
    *)
        printf 'unsupported profiling platform: %s\n' "$platform" >&2
        status=2
        ;;
esac
set -e

printf 'status\t%s\n' "$status" >>"$output/run.meta"
printf 'finished\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$output/run.meta"

find "$language_repo/src/fst" -type f \
    \( -name '*.hfst' -o -name '*.hfstol' -o -name '*.pmhfst' \) \
    -exec sh -c '
        root=$1
        shift
        for artifact do
            relative=${artifact#"$root"/}
            bytes=$(wc -c <"$artifact" | tr -d " ")
            printf "%s\t%s\n" "$relative" "$bytes"
        done
    ' sh "$language_repo" {} + | LC_ALL=C sort >"$output/artifacts.tsv"

if [ "$status" -eq 0 ] && [ -n "${HFST_PROFILE_BASELINE_DIR-}" ]; then
    compared=0
    while IFS= read -r current; do
        [ -n "$current" ] || continue
        relative=${current#"$language_repo"/}
        baseline=$HFST_PROFILE_BASELINE_DIR/$relative
        if [ ! -f "$baseline" ]; then
            printf 'baseline artifact is missing: %s\n' "$baseline" >&2
            status=1
            break
        fi
        if ! "$actual_dir/hfst-compare" -s "$baseline" "$current"; then
            printf 'semantic comparison failed: %s\n' "$relative" >&2
            status=1
            break
        fi
        compared=$((compared + 1))
    done <<EOF
$(find "$language_repo/src/fst" -type f -name '*.hfst' | LC_ALL=C sort)
EOF
    printf 'semantically_compared_hfst_files\t%s\n' "$compared" \
        >>"$output/run.meta"
fi

printf 'profile written to %s\n' "$output"
exit "$status"
