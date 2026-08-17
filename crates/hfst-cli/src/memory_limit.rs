//! Memory-allowance policy for `hfst-compose`.
//!
//! The CLI resolves this once, before it loads either input transducer. The
//! library receives only the resulting byte allowance, so platform probing and
//! environment-variable policy stay at the application boundary.

use sysinfo::{MemoryRefreshKind, RefreshKind, System};

pub(crate) const MEMORY_LIMIT_ENV: &str = "HFST_COMPOSE_MEMORY_LIMIT";

/// Host and container headroom observed at one instant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MemorySnapshot {
    pub(crate) host_available_bytes: Option<u64>,
    pub(crate) cgroup_headroom_bytes: Option<u64>,
}

impl MemorySnapshot {
    /// Read RAM data only. In particular, this does not enumerate processes or
    /// count swap as memory available to composition.
    pub(crate) fn capture() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        );

        // `System::cgroup_limits` requires a successful memory refresh and
        // asserts that total memory is non-zero. A zero total is sysinfo's
        // unsupported/probe-failed value, not a reason to panic here.
        if system.total_memory() == 0 {
            return Self::default();
        }

        Self {
            host_available_bytes: Some(system.available_memory()),
            cgroup_headroom_bytes: system.cgroup_limits().map(|limits| limits.free_memory),
        }
    }

    /// Available physical memory, constrained by a container when one is
    /// present. Missing observations do not erase a usable observation from
    /// the other source.
    pub(crate) fn effective_available_bytes(self) -> Option<u64> {
        match (self.host_available_bytes, self.cgroup_headroom_bytes) {
            (Some(host), Some(cgroup)) => Some(host.min(cgroup)),
            (Some(host), None) => Some(host),
            (None, Some(cgroup)) => Some(cgroup),
            (None, None) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LimitSource {
    Cli,
    Environment,
    Automatic,
    ProbeFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedMemoryLimit {
    /// The exact allowance passed to the composition engine. Its internal
    /// allocator accounting may retain part of this as safety headroom.
    pub(crate) allowance_bytes: u64,
    pub(crate) source: LimitSource,
    /// Present only when an operator supplied a CLI or environment value.
    pub(crate) requested_bytes: Option<u64>,
    /// True when a requested value exceeded current cgroup headroom.
    pub(crate) cgroup_clamped: bool,
}

/// Parse an exact, integer memory quantity.
///
/// Bare values and `B` are bytes. `K`/`KB`/`KiB` through
/// `T`/`TB`/`TiB` are binary multiples (powers of 1024), case-insensitively.
/// Zero is deliberately valid: it forces the budgeted store to spill from its
/// first allocation.
pub(crate) fn parse_size(input: &str) -> Result<u64, String> {
    let value = input.trim();
    if value.is_empty() {
        return Err("memory size is empty".to_string());
    }

    let digits_end = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    if digits_end == 0 {
        return Err(format!(
            "memory size '{value}' must start with an unsigned integer"
        ));
    }

    let number = value[..digits_end]
        .parse::<u64>()
        .map_err(|_| format!("memory size '{value}' is too large"))?;
    let unit = value[digits_end..].to_ascii_uppercase();
    let multiplier = match unit.as_str() {
        "" | "B" => 1,
        "K" | "KB" | "KIB" => 1_u64 << 10,
        "M" | "MB" | "MIB" => 1_u64 << 20,
        "G" | "GB" | "GIB" => 1_u64 << 30,
        "T" | "TB" | "TIB" => 1_u64 << 40,
        _ => {
            return Err(format!(
                "unknown memory-size unit '{}' in '{value}'",
                &value[digits_end..]
            ));
        }
    };

    number
        .checked_mul(multiplier)
        .ok_or_else(|| format!("memory size '{value}' is too large"))
}

/// Pure precedence and clamping logic. `environment` is parsed only when no
/// CLI value was supplied, so a stale or malformed environment override cannot
/// defeat an explicit invocation.
pub(crate) fn resolve_from(
    cli: Option<u64>,
    environment: Option<&str>,
    snapshot: MemorySnapshot,
) -> Result<ResolvedMemoryLimit, String> {
    let (selected, source, requested_bytes) = if let Some(bytes) = cli {
        (bytes, LimitSource::Cli, Some(bytes))
    } else if let Some(value) = environment {
        let bytes = parse_size(value)
            .map_err(|detail| format!("invalid {MEMORY_LIMIT_ENV} value: {detail}"))?;
        (bytes, LimitSource::Environment, Some(bytes))
    } else if let Some(available) = snapshot.effective_available_bytes() {
        (available / 2, LimitSource::Automatic, None)
    } else {
        (0, LimitSource::ProbeFallback, None)
    };

    // An explicit value overrides the host-availability heuristic, but it must
    // not ask a process to cross a hard container boundary. The automatic value
    // is already at most half the cgroup headroom and therefore needs no clamp.
    let allowance_bytes = if requested_bytes.is_some() {
        snapshot
            .cgroup_headroom_bytes
            .map_or(selected, |headroom| selected.min(headroom))
    } else {
        selected
    };

    Ok(ResolvedMemoryLimit {
        allowance_bytes,
        source,
        requested_bytes,
        cgroup_clamped: requested_bytes.is_some_and(|requested| requested > allowance_bytes),
    })
}

/// Resolve the process environment and a single startup memory snapshot.
pub(crate) fn resolve(cli: Option<u64>) -> Result<ResolvedMemoryLimit, String> {
    let environment = if cli.is_some() {
        None
    } else {
        match std::env::var_os(MEMORY_LIMIT_ENV) {
            Some(value) => Some(
                value
                    .into_string()
                    .map_err(|_| format!("{MEMORY_LIMIT_ENV} is not valid Unicode"))?,
            ),
            None => None,
        }
    };

    resolve_from(cli, environment.as_deref(), MemorySnapshot::capture())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIB: u64 = 1 << 10;
    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;
    const TIB: u64 = 1 << 40;

    #[test]
    fn parses_integer_binary_sizes_and_zero() {
        for (text, expected) in [
            ("0", 0),
            ("17", 17),
            ("17B", 17),
            ("18446744073709551615", u64::MAX),
            ("2K", 2 * KIB),
            ("2kb", 2 * KIB),
            ("2KiB", 2 * KIB),
            ("3m", 3 * MIB),
            ("3MB", 3 * MIB),
            ("3mib", 3 * MIB),
            ("4G", 4 * GIB),
            ("4gb", 4 * GIB),
            ("4GiB", 4 * GIB),
            ("5T", 5 * TIB),
            ("5tb", 5 * TIB),
            ("5TiB", 5 * TIB),
            (" 6MiB ", 6 * MIB),
        ] {
            assert_eq!(parse_size(text), Ok(expected), "input {text:?}");
        }
    }

    #[test]
    fn rejects_inexact_malformed_and_overflowing_sizes() {
        for text in [
            "",
            "   ",
            "-1",
            "+1",
            "1.5GiB",
            "KiB",
            "1 GiB",
            "1XB",
            "18446744073709551616",
            "18446744073709551615KiB",
        ] {
            assert!(parse_size(text).is_err(), "accepted {text:?}");
        }
    }

    #[test]
    fn effective_available_uses_the_lower_present_headroom() {
        assert_eq!(
            MemorySnapshot {
                host_available_bytes: Some(101),
                cgroup_headroom_bytes: Some(80),
            }
            .effective_available_bytes(),
            Some(80)
        );
        assert_eq!(
            MemorySnapshot {
                host_available_bytes: Some(101),
                cgroup_headroom_bytes: None,
            }
            .effective_available_bytes(),
            Some(101)
        );
        assert_eq!(
            MemorySnapshot {
                host_available_bytes: None,
                cgroup_headroom_bytes: Some(80),
            }
            .effective_available_bytes(),
            Some(80)
        );
        assert_eq!(MemorySnapshot::default().effective_available_bytes(), None);
    }

    #[test]
    fn cli_beats_env_without_host_clamp() {
        let resolved = resolve_from(
            Some(400),
            Some("not-a-size"),
            MemorySnapshot {
                host_available_bytes: Some(100),
                cgroup_headroom_bytes: None,
            },
        )
        .expect("CLI value makes the malformed environment value irrelevant");

        assert_eq!(
            resolved,
            ResolvedMemoryLimit {
                allowance_bytes: 400,
                source: LimitSource::Cli,
                requested_bytes: Some(400),
                cgroup_clamped: false,
            }
        );
    }

    #[test]
    fn environment_beats_the_automatic_default() {
        let resolved = resolve_from(
            None,
            Some("2KiB"),
            MemorySnapshot {
                host_available_bytes: Some(100),
                cgroup_headroom_bytes: None,
            },
        )
        .expect("valid environment value");

        assert_eq!(resolved.allowance_bytes, 2 * KIB);
        assert_eq!(resolved.source, LimitSource::Environment);
        assert_eq!(resolved.requested_bytes, Some(2 * KIB));
        assert!(!resolved.cgroup_clamped);
    }

    #[test]
    fn automatic_default_is_half_effective_available_rounded_down() {
        for (snapshot, expected) in [
            (
                MemorySnapshot {
                    host_available_bytes: Some(101),
                    cgroup_headroom_bytes: None,
                },
                50,
            ),
            (
                MemorySnapshot {
                    host_available_bytes: None,
                    cgroup_headroom_bytes: Some(81),
                },
                40,
            ),
            (
                MemorySnapshot {
                    host_available_bytes: Some(101),
                    cgroup_headroom_bytes: Some(80),
                },
                40,
            ),
        ] {
            let resolved = resolve_from(None, None, snapshot).expect("automatic resolution");
            assert_eq!(resolved.allowance_bytes, expected);
            assert_eq!(resolved.source, LimitSource::Automatic);
            assert_eq!(resolved.requested_bytes, None);
            assert!(!resolved.cgroup_clamped);
        }
    }

    #[test]
    fn missing_probe_data_falls_back_to_zero() {
        assert_eq!(
            resolve_from(None, None, MemorySnapshot::default()).expect("safe fallback"),
            ResolvedMemoryLimit {
                allowance_bytes: 0,
                source: LimitSource::ProbeFallback,
                requested_bytes: None,
                cgroup_clamped: false,
            }
        );
    }

    #[test]
    fn explicit_values_are_clamped_to_cgroup_headroom() {
        for (cli, environment, source) in [
            (Some(500), None, LimitSource::Cli),
            (None, Some("500"), LimitSource::Environment),
        ] {
            let resolved = resolve_from(
                cli,
                environment,
                MemorySnapshot {
                    host_available_bytes: Some(1_000),
                    cgroup_headroom_bytes: Some(300),
                },
            )
            .expect("valid explicit value");

            assert_eq!(resolved.allowance_bytes, 300);
            assert_eq!(resolved.source, source);
            assert_eq!(resolved.requested_bytes, Some(500));
            assert!(resolved.cgroup_clamped);
        }
    }

    #[test]
    fn malformed_selected_environment_value_is_an_error() {
        let error = resolve_from(
            None,
            Some("1.5GiB"),
            MemorySnapshot {
                host_available_bytes: Some(1_000),
                cgroup_headroom_bytes: None,
            },
        )
        .expect_err("fractional environment value must fail");
        assert!(error.contains(MEMORY_LIMIT_ENV));
        assert!(error.contains("1.5GiB"));
    }

    #[test]
    fn production_resolver_accepts_cli_zero() {
        let resolved = resolve(Some(0)).expect("zero is a valid forced-spill allowance");
        assert_eq!(resolved.allowance_bytes, 0);
        assert_eq!(resolved.source, LimitSource::Cli);
        assert_eq!(resolved.requested_bytes, Some(0));
        assert!(!resolved.cgroup_clamped);
    }

    #[test]
    fn platform_capture_is_non_panicking() {
        let _snapshot = MemorySnapshot::capture();
    }
}
