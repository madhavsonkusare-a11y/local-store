//! Read-only CLI commands sharing the GUI's catalog and Doctor data models.
use local_store::{brand::CLI_NAME, catalog, runtime};
use pico_args::Arguments;
use serde::Serialize;
use std::fmt::Write;

pub(super) struct Output {
    pub text: String,
    pub code: i32,
}

pub(super) fn recovery(values: &[String]) -> Result<Output, String> {
    let mut args = arguments(values);
    if args.contains(["-h", "--help"]) {
        return Ok(Output { text: format!("Usage: {CLI_NAME} recovery [--json] [--docker]\nRead-only retained setup inventory; --docker checks Compose ownership labels. Never changes apps.\n"), code: 0 });
    }
    let json = args.contains("--json");
    let docker = args.contains("--docker");
    if !args.finish().is_empty() {
        return Err(format!("Usage: {CLI_NAME} recovery [--json] [--docker]"));
    }
    let mut candidates = local_store::recovery::inspect().map_err(|error| error.to_string())?;
    if docker {
        for candidate in &mut candidates {
            local_store::recovery::verify_with(candidate, &runtime::SystemProcessRunner)
                .map_err(|error| error.to_string())?;
        }
    }
    let text = if json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&candidates).map_err(|error| error.to_string())?
        )
    } else if candidates.is_empty() {
        "No unregistered setup files found for supported recipes.\n".into()
    } else {
        let mut text =
            "Retained setup files (a snapshot, not permission to stop or delete apps):\n"
                .to_owned();
        for candidate in candidates {
            writeln!(
                &mut text,
                "{} [{:?}]: {}",
                candidate.recipe_id,
                candidate.ownership_status,
                candidate.compose_file.display()
            )
            .unwrap();
        }
        text
    };
    Ok(Output { text, code: 0 })
}

fn arguments(values: &[String]) -> Arguments {
    Arguments::from_vec(values.iter().map(Into::into).collect())
}

fn value(args: &mut Arguments, key: &'static str) -> Result<Option<String>, String> {
    let value: Option<String> = args
        .opt_value_from_str(key)
        .map_err(|e| format!("{key}: {e}"))?;
    if value
        .as_ref()
        .is_some_and(|value| value.trim().is_empty() || value.starts_with('-'))
    {
        return Err(format!("{key} requires a value"));
    }
    Ok(value)
}

fn number(args: &mut Arguments, key: &'static str, default: usize) -> Result<usize, String> {
    match value(args, key)? {
        None => Ok(default),
        Some(value) if value.bytes().all(|byte| byte.is_ascii_digit()) => {
            value.parse().map_err(|_| format!("{key} is too large"))
        }
        Some(_) => Err(format!("{key} requires a non-negative integer")),
    }
}

pub(super) fn doctor(
    values: &[String],
    diagnose: impl FnOnce() -> runtime::DoctorReport,
) -> Result<Output, String> {
    let mut args = arguments(values);
    if args.contains(["-h", "--help"]) {
        return Ok(Output { text: format!("Usage: {CLI_NAME} doctor [--json]\nExit: 0 ready, 1 prerequisites unavailable, 2 invalid arguments.\n"), code: 0 });
    }
    let json = args.contains("--json");
    if !args.finish().is_empty() {
        return Err(format!("Usage: {CLI_NAME} doctor [--json]"));
    }
    let report = diagnose();
    let text = if json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        )
    } else {
        report
            .checks
            .iter()
            .map(|check| {
                format!(
                    "{} {:<18} {}\n",
                    if check.ok { "ok" } else { "fail" },
                    check.label,
                    check.detail
                )
            })
            .collect()
    };
    Ok(Output {
        text,
        code: i32::from(!report.ready),
    })
}

#[derive(Serialize)]
struct CatalogResult {
    #[serde(flatten)]
    page: catalog::CatalogPage,
    next_offset: Option<usize>,
}

pub(super) fn catalog(values: &[String]) -> Result<Output, String> {
    let split = values
        .iter()
        .position(|value| value == "--")
        .unwrap_or(values.len());
    let mut args = arguments(&values[..split]);
    if args.contains(["-h", "--help"]) {
        return Ok(Output {
            text: format!(
                "Usage: {CLI_NAME} catalog [search words] [options]\n\n\
             --query <text>          Search IDs, names, aliases, descriptions and tags\n\
             --category <name>       Exact category\n\
             --capability <value>    preview_install, connect or discover\n\
             --license <name>        Exact software license\n\
             --architecture <name>   Exact container architecture\n\
             --collection <value>    writing, automation, media or developer\n\
             --hide-warnings        Exclude entries with catalog cautions\n\
             --offset <number>      Zero-based offset (default 0)\n\
             --limit <number>       Page size, 1–48 (default 24)\n\
             --json                 Print a CatalogPage with next_offset\n\
             --                     Treat following words as literal search text\n\n\
             Catalog results describe projects, not running app addresses.\n"
            ),
            code: 0,
        });
    }
    let json = args.contains("--json");
    let query = value(&mut args, "--query")?;
    let category = value(&mut args, "--category")?.unwrap_or_default();
    let filters = catalog::Filters {
        capability: value(&mut args, "--capability")?.unwrap_or_default(),
        license: value(&mut args, "--license")?.unwrap_or_default(),
        architecture: value(&mut args, "--architecture")?.unwrap_or_default(),
        collection: value(&mut args, "--collection")?.unwrap_or_default(),
        hide_warnings: args.contains("--hide-warnings"),
    };
    if !["", "preview_install", "connect", "discover"].contains(&filters.capability.as_str()) {
        return Err("--capability must be preview_install, connect or discover".into());
    }
    if !["", "writing", "automation", "media", "developer"].contains(&filters.collection.as_str()) {
        return Err("--collection must be writing, automation, media or developer".into());
    }
    let offset = number(&mut args, "--offset", 0)?;
    let limit = number(&mut args, "--limit", 24)?;
    if !(1..=48).contains(&limit) {
        return Err("--limit must be between 1 and 48".into());
    }
    let mut words = Vec::new();
    for word in args.finish() {
        let word = word
            .into_string()
            .map_err(|_| "Search text must be UTF-8")?;
        if word.starts_with('-') {
            return Err(format!(
                "Unknown or repeated option: {word}. Use catalog --help."
            ));
        }
        words.push(word);
    }
    if split < values.len() {
        words.extend_from_slice(&values[split + 1..]);
    }
    if query.is_some() && !words.is_empty() {
        return Err("Use search words or --query, not both".into());
    }
    let query = query.unwrap_or_else(|| words.join(" "));
    let page = catalog::search_filtered(&query, &category, offset, limit, &filters);
    let end = page.offset.saturating_add(page.entries.len());
    let next_offset = (end < page.total).then_some(end);
    let result = CatalogResult { page, next_offset };
    let text = if json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
        )
    } else {
        let page = &result.page;
        let mut text = format!(
            "Local Store catalog: {} matches / {} projects · snapshot {}\n",
            page.total, page.catalog_total, page.snapshot_date
        );
        if page.entries.is_empty() {
            let _ = writeln!(
                text,
                "No projects at offset {offset}. Try another search or --offset 0."
            );
        } else {
            let _ = writeln!(text, "Showing {}–{}\n", page.offset + 1, end);
            let _ = writeln!(text, "{:<30} {:<28} ACTION", "ID", "NAME");
            for entry in &page.entries {
                let _ = writeln!(
                    text,
                    "{:<30} {:<28} {}\n    {}",
                    entry.id, entry.name, entry.capability, entry.source_url
                );
            }
        }
        if let Some(next) = result.next_offset {
            let _ = writeln!(text, "Next page: repeat the same search and filters with --offset {next} --limit {limit}.");
        }
        text
    };
    Ok(Output { text, code: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeRunner {
        healthy: bool,
        calls: AtomicUsize,
    }
    impl runtime::ProcessRunner for FakeRunner {
        fn run_cancellable(
            &self,
            _: &runtime::CommandSpec,
            _: &runtime::CancelToken,
        ) -> Result<runtime::ProcessOutput, runtime::ProcessError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.healthy {
                Ok(runtime::ProcessOutput {
                    success: true,
                    stdout: "1.2.3\n".into(),
                    stderr: String::new(),
                    truncated: false,
                })
            } else {
                Err(runtime::ProcessError::new(
                    runtime::ProcessErrorCode::ProcessUnavailable,
                    "Docker unavailable\nCheck \"installation\"",
                ))
            }
        }
    }
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }

    #[test]
    fn doctor_json_preserves_both_checks_and_readiness_exit_codes() {
        for healthy in [true, false] {
            let runner = FakeRunner {
                healthy,
                calls: AtomicUsize::new(0),
            };
            let output = doctor(&args(&["--json"]), || runtime::doctor_with(&runner)).unwrap();
            let report: serde_json::Value = serde_json::from_str(&output.text).unwrap();
            assert_eq!(report["ready"], healthy);
            if healthy {
                assert!(report["checks"][0].get("error").is_none());
            } else {
                assert_eq!(report["checks"][0]["error"]["code"], "process_unavailable");
                assert_eq!(
                    report["checks"][0]["error"]["message"],
                    report["checks"][0]["detail"]
                );
            }
            assert_eq!(output.code, i32::from(!healthy));
            assert_eq!(runner.calls.load(Ordering::SeqCst), 2);
            assert_eq!(report["checks"][0]["id"], "docker");
            assert_eq!(report["checks"][1]["id"], "compose");
            assert_eq!(
                report["checks"][0]["detail"],
                if healthy {
                    "1.2.3"
                } else {
                    "Docker unavailable\nCheck \"installation\""
                }
            );
            let text = doctor(&[], || runtime::doctor_with(&runner)).unwrap();
            assert!(text.text.contains(if healthy { "ok" } else { "fail" }));
        }
    }

    #[test]
    fn doctor_help_and_argument_errors_do_not_invoke_docker() {
        let runner = FakeRunner {
            healthy: true,
            calls: AtomicUsize::new(0),
        };
        for values in [
            &["--json", "--json"][..],
            &["--unknown"],
            &["extra"],
            &["--json=true"],
        ] {
            assert!(doctor(&args(values), || runtime::doctor_with(&runner)).is_err());
        }
        assert_eq!(
            doctor(&args(&["--help"]), || runtime::doctor_with(&runner))
                .unwrap()
                .code,
            0
        );
        assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn catalog_query_forms_use_the_shared_index_and_filters() {
        let values = args(&[
            "--query=paperless",
            "--license",
            "GPL-3.0",
            "--collection",
            "writing",
            "--hide-warnings",
            "--limit",
            "2",
            "--json",
        ]);
        let actual: serde_json::Value =
            serde_json::from_str(&catalog(&values).unwrap().text).unwrap();
        let expected = catalog::search_filtered(
            "paperless",
            "",
            0,
            2,
            &catalog::Filters {
                license: "GPL-3.0".into(),
                collection: "writing".into(),
                hide_warnings: true,
                ..Default::default()
            },
        );
        assert!(expected.total > 0);
        assert_eq!(
            actual["entries"],
            serde_json::to_value(expected).unwrap()["entries"]
        );
        let a = catalog(&args(&["--json", "home", "assistant"]))
            .unwrap()
            .text;
        let b = catalog(&args(&["--json", "--query", "home assistant"]))
            .unwrap()
            .text;
        assert_eq!(a, b);
        assert!(catalog(&args(&["--json", "--", "--literal"])).is_ok());
    }

    #[test]
    fn catalog_rejects_bad_ranges_missing_values_and_unknown_options() {
        for values in [
            vec!["--limit", "0"],
            vec!["--limit=49"],
            vec!["--offset", "-1"],
            vec!["--limit", "many"],
            vec!["--offset", "999999999999999999999999"],
            vec!["--limit"],
            vec!["--license", "--json"],
            vec!["--json", "--json"],
            vec!["--capability", "install"],
            vec!["--collection", "unknown"],
            vec!["--offset", "0", "--offset", "1"],
            vec!["--unknown"],
            vec!["--query", "one", "two"],
        ] {
            assert!(catalog(&args(&values)).is_err(), "accepted {values:?}");
        }
    }
}
