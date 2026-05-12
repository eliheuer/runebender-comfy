// check-crate-age — supply-chain cooldown for Cargo.lock.
//
// Mirrors the `minimum-release-age` pnpm setting on the Rust side.
// Cargo has no install-time hook, so this runs *after* dependencies
// resolve — call it from a pre-commit hook or CI gate.
//
// Usage:
//
//     check-crate-age [PATH_TO_CARGO_LOCK]
//         [--days N]                  cooldown in days (default 7)
//         [--exclude name,name,...]   crates that bypass the cooldown
//
// Exit codes:
//     0  every crates.io-sourced crate is at least N days old
//     1  one or more crates are too fresh
//     2  bad CLI args / can't read Cargo.lock / network failure

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use serde::Deserialize;

const DEFAULT_COOLDOWN_DAYS: i64 = 7;
const CRATES_IO_REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const USER_AGENT: &str =
    "check-crate-age (https://github.com/eliheuer/runebender-comfy)";

// ============================================================================
// Cargo.lock + crates.io shapes
// ============================================================================

#[derive(Debug, Deserialize)]
struct CargoLock {
    package: Vec<LockPackage>,
}

#[derive(Debug, Deserialize)]
struct LockPackage {
    name: String,
    version: String,
    /// Absent for path/workspace members; present for registry and
    /// git deps. Registry deps look like
    /// "registry+https://github.com/rust-lang/crates.io-index".
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CratesApiResponse {
    version: CratesApiVersion,
}

#[derive(Debug, Deserialize)]
struct CratesApiVersion {
    /// RFC3339 timestamp of when this specific version was published.
    created_at: String,
}

// ============================================================================
// CLI parsing — minimal, no clap to keep build fast
// ============================================================================

struct Args {
    lock_path: PathBuf,
    cooldown_days: i64,
    excluded: HashSet<String>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut lock_path: Option<PathBuf> = None;
        let mut cooldown_days = DEFAULT_COOLDOWN_DAYS;
        let mut excluded: HashSet<String> = HashSet::new();

        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--days" => {
                    let n = it.next().ok_or("--days requires a value")?;
                    cooldown_days =
                        n.parse().map_err(|_| format!("--days: bad number `{n}`"))?;
                }
                "--exclude" => {
                    let csv = it.next().ok_or("--exclude requires a value")?;
                    for name in csv.split(',') {
                        let name = name.trim();
                        if !name.is_empty() {
                            excluded.insert(name.to_string());
                        }
                    }
                }
                "-h" | "--help" => {
                    print_usage();
                    std::process::exit(0);
                }
                other if other.starts_with("--") => {
                    return Err(format!("unknown flag: {other}"));
                }
                _ => {
                    if lock_path.is_some() {
                        return Err(format!("unexpected positional arg: {arg}"));
                    }
                    lock_path = Some(PathBuf::from(arg));
                }
            }
        }

        Ok(Self {
            lock_path: lock_path.unwrap_or_else(|| PathBuf::from("Cargo.lock")),
            cooldown_days,
            excluded,
        })
    }
}

fn print_usage() {
    eprintln!(
        "check-crate-age — supply-chain cooldown for Cargo.lock\n\
        \n\
        Usage:\n\
        \tcheck-crate-age [PATH_TO_CARGO_LOCK]\n\
        \t                [--days N]\n\
        \t                [--exclude name,name,...]\n\
        \n\
        Defaults:\n\
        \tPATH_TO_CARGO_LOCK = Cargo.lock\n\
        \t--days             = {DEFAULT_COOLDOWN_DAYS}"
    );
}

// ============================================================================
// main
// ============================================================================

fn main() -> ExitCode {
    let args = match Args::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            print_usage();
            return ExitCode::from(2);
        }
    };

    let lock_text = match fs::read_to_string(&args.lock_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: can't read {}: {e}", args.lock_path.display());
            return ExitCode::from(2);
        }
    };

    let lock: CargoLock = match toml::from_str(&lock_text) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: can't parse {}: {e}", args.lock_path.display());
            return ExitCode::from(2);
        }
    };

    let agent: ureq::Agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(15))
        .timeout_connect(Duration::from_secs(10))
        .user_agent(USER_AGENT)
        .build();

    let now = chrono::Utc::now();
    let cooldown = chrono::Duration::days(args.cooldown_days);

    let mut checked = 0usize;
    let mut skipped_non_crates_io = 0usize;
    let mut excluded = 0usize;
    let mut warnings = 0usize;
    let mut failures: Vec<(String, String, i64)> = Vec::new();

    for pkg in &lock.package {
        // Workspace members + path deps have no source.
        let Some(source) = &pkg.source else {
            continue;
        };
        if source != CRATES_IO_REGISTRY {
            skipped_non_crates_io += 1;
            continue;
        }
        if args.excluded.contains(&pkg.name) {
            excluded += 1;
            continue;
        }

        let url = format!(
            "https://crates.io/api/v1/crates/{}/{}",
            pkg.name, pkg.version,
        );

        let resp = match agent.get(&url).call() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warn: query failed for {}@{}: {e}", pkg.name, pkg.version);
                warnings += 1;
                continue;
            }
        };

        let parsed: CratesApiResponse = match resp.into_json() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("warn: bad response for {}@{}: {e}", pkg.name, pkg.version);
                warnings += 1;
                continue;
            }
        };

        let created = match chrono::DateTime::parse_from_rfc3339(&parsed.version.created_at) {
            Ok(d) => d.with_timezone(&chrono::Utc),
            Err(e) => {
                eprintln!(
                    "warn: can't parse created_at for {}@{}: {e}",
                    pkg.name, pkg.version,
                );
                warnings += 1;
                continue;
            }
        };

        let age = now - created;
        checked += 1;

        if age < cooldown {
            failures.push((pkg.name.clone(), pkg.version.clone(), age.num_days()));
        }
    }

    eprintln!(
        "checked {checked} crates.io crates, skipped {skipped_non_crates_io} non-registry, {excluded} excluded, {warnings} warnings"
    );

    if failures.is_empty() {
        eprintln!("✓ all crates ≥ {} days old", args.cooldown_days);
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "✗ {} crate(s) younger than {} days:",
            failures.len(),
            args.cooldown_days,
        );
        for (name, version, days) in &failures {
            eprintln!("  {name} {version} ({days} days)");
        }
        eprintln!(
            "\nto bypass for a specific crate (only after auditing it), run:\n  \
            check-crate-age --exclude {} ...",
            failures.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>().join(","),
        );
        ExitCode::FAILURE
    }
}
