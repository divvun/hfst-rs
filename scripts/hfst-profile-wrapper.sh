#!/bin/sh

set -u

tool=$(basename "$0")
actual_dir=${HFST_PROFILE_ACTUAL_DIR:?HFST_PROFILE_ACTUAL_DIR is required}
log_dir=${HFST_PROFILE_LOG_DIR:?HFST_PROFILE_LOG_DIR is required}
run_id=$(date -u +%Y%m%dT%H%M%S)-$$
base=$log_dir/$run_id-$tool

mkdir -p "$log_dir"
{
    printf 'tool\t%s\n' "$tool"
    printf 'cwd\t%s\n' "$PWD"
    printf 'started\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    for argument in "$@"; do
        printf 'argument\t%s\n' "$argument"
    done
} >"$base.meta"

platform=$(uname -s)
set +e
case $platform in
    Darwin)
        /usr/bin/time -l -o "$base.time" "$actual_dir/$tool" "$@"
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
        ' "$base.time" >"$base.metrics"
        ;;
    Linux)
        /usr/bin/time -f 'wall_seconds\t%e\nuser_seconds\t%U\nsystem_seconds\t%S\nmax_rss_kib\t%M' \
            -o "$base.time" "$actual_dir/$tool" "$@"
        status=$?
        awk -F '\t' '
            { print }
            $1 == "max_rss_kib" {
                printf "max_rss_bytes\t%.0f\n", $2 * 1024
            }
        ' "$base.time" >"$base.metrics"
        ;;
    *)
        printf 'unsupported profiling platform: %s\n' "$platform" >&2
        status=2
        ;;
esac
set -e

{
    printf 'status\t%s\n' "$status"
    printf 'finished\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$base.meta"

exit "$status"
