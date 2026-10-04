//! Opt-in existing-host proof of the production removal transaction in a
//! strictly isolated test namespace. Never a fixed-name clean-host claim.
use super::*;
use crate::runtime::{
    CancelToken, ProcessError, ProcessErrorCode, ProcessOutput, SystemProcessRunner,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const BYTES: u64 = 572_798_976;
const HASH: &str = "ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32";

struct Namespace {
    fixture: String,
    deadline: Instant,
    calls: Mutex<Vec<Vec<String>>>,
}
impl ProcessRunner for Namespace {
    fn run_cancellable(
        &self,
        command: &CommandSpec,
        cancel: &CancelToken,
    ) -> Result<ProcessOutput, ProcessError> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                ProcessError::new(
                    ProcessErrorCode::TimedOut,
                    "Removal fixture overall deadline expired; retain its ownership records.",
                )
            })?;
        if command.program != "wsl.exe" {
            return Err(ProcessError::new(
                ProcessErrorCode::ProcessFailed,
                "Fixture accepts explicit WSL commands only.",
            ));
        }
        let mut actual = command.clone();
        actual.timeout = actual.timeout.min(remaining);
        let action = actual.args.first().map(String::as_str).unwrap_or_default();
        let list = match action {
            "--list"
                if actual.args == ["--list", "--quiet"]
                    || actual.args == ["--list", "--verbose"] =>
            {
                true
            }
            "--import" | "--unregister" | "--distribution"
                if actual.args.get(1).map(String::as_str) == Some(wsl::DISTRO) =>
            {
                // No config/environment override: only this private test adapter
                // translates the one fixed product distro argument position.
                actual.args[1] = self.fixture.clone();
                false
            }
            _ => {
                return Err(ProcessError::new(
                    ProcessErrorCode::ProcessFailed,
                    "Fixture refused an unsupported command or distro argument.",
                ))
            }
        };
        if actual
            .args
            .iter()
            .any(|a| a == wsl::DISTRO || a == "--shutdown" || a == "--terminate")
        {
            return Err(ProcessError::new(
                ProcessErrorCode::ProcessFailed,
                "Production distro/global WSL commands are forbidden in the fixture.",
            ));
        }
        self.calls.lock().unwrap().push(actual.args.clone());
        let mut out = SystemProcessRunner.run_cancellable(&actual, cancel)?;
        if list && out.success && !out.truncated && out.stderr.is_empty() {
            if actual.args[1] == "--quiet" {
                let names = bootstrap::parse_distro_names(&out.stdout).map_err(|e| {
                    ProcessError::new(ProcessErrorCode::ProcessFailed, e.to_string())
                })?;
                out.stdout = if names.iter().any(|n| n == &self.fixture) {
                    format!("{}\n", wsl::DISTRO)
                } else {
                    String::new()
                };
            } else {
                // Scope the verbose inventory to this unique fixture only.
                // Product-name and unrelated rows can never masquerade as it.
                let decoded = out.stdout.replace('\0', "");
                let rows: Vec<_> = decoded
                    .lines()
                    .filter_map(|line| {
                        let fields: Vec<_> = line
                            .trim()
                            .trim_start_matches('*')
                            .split_whitespace()
                            .collect();
                        (fields.first() == Some(&self.fixture.as_str())).then_some(fields)
                    })
                    .collect();
                if rows.len() != 1 || rows[0].len() != 3 || rows[0][2] != "2" {
                    return Err(ProcessError::new(
                        ProcessErrorCode::ProcessFailed,
                        "Unique fixture WSL 2 inventory is missing or ambiguous.",
                    ));
                }
                out.stdout = format!("{} {} 2\n", wsl::DISTRO, rows[0][1]);
            }
        }
        Ok(out)
    }
}

fn real_inventory() -> AppResult<Vec<String>> {
    let mut spec = CommandSpec::new(
        "wsl.exe",
        vec!["--list".into(), "--quiet".into()],
        None,
        DIAGNOSTIC_TIMEOUT,
    );
    spec.remove_env.push("WSLENV".into());
    let mut names = bootstrap::parse_distro_names(&checked_output(&SystemProcessRunner, &spec)?)?;
    names.sort();
    Ok(names)
}
fn native_state() -> AppResult<Vec<(String, Option<Vec<u8>>)>> {
    [
        bootstrap::JOURNAL_FILE,
        bootstrap::TOKEN_FILE,
        "selected-engine.json",
        "engine-selection-configured",
    ]
    .iter()
    .map(|name| {
        let bytes = match fs::read(storage::managed_engine_state_root().join(name)) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        Ok(((*name).into(), bytes))
    })
    .collect()
}
fn command(args: &[&str]) -> CommandSpec {
    let mut spec = CommandSpec::new(
        "wsl.exe",
        vec![
            "--distribution".into(),
            wsl::DISTRO.into(),
            "--user".into(),
            "root".into(),
            "--exec".into(),
        ],
        None,
        DIAGNOSTIC_TIMEOUT,
    );
    spec.args.extend(args.iter().map(|a| (*a).into()));
    spec.remove_env.push("WSLENV".into());
    spec
}

#[test]
#[ignore = "Requires explicit LOCAL_STORE_RUN_ENGINE_REMOVAL_PROOF=1; imports and removes only a new unique owned fixture"]
fn actual_unique_fixture_removal_transaction() {
    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_ENGINE_REMOVAL_PROOF")
            .ok()
            .as_deref(),
        Some("1"),
        "Explicit opt-in is required; no engine change was made."
    );
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let native_before = native_state().unwrap();
    let inventory_before = real_inventory().unwrap();
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let fixture = format!("local-store-removal-proof-{}-{millis}", std::process::id());
    assert!(!inventory_before
        .iter()
        .any(|n| n.eq_ignore_ascii_case(&fixture)));
    let root = repo.join("target/engine-removal-proof").join(&fixture);
    assert!(!root.exists());
    checked_tree(root.parent().unwrap()).unwrap();
    fs::create_dir_all(&root).unwrap();
    let paths = Paths {
        state: root.join("engine/wsl-state"),
        data: root.join("engine/wsl-data"),
        apps: root.join("config/local-store/apps"),
        registry: root.join("config"),
    };
    let marker = root.join("fixture-owner.json");
    fs::write(
        &marker,
        serde_json::to_vec_pretty(
            &json!({"schema_version":1,"fixture":fixture,"data":paths.data,"rootfs_sha256":HASH}),
        )
        .unwrap(),
    )
    .unwrap();
    fs::create_dir_all(&paths.apps).unwrap();
    let rootfs = repo.join("target/release/engine/rootfs.tar");
    let runner = Namespace {
        fixture: fixture.clone(),
        deadline: Instant::now() + Duration::from_secs(300),
        calls: Mutex::new(Vec::new()),
    };
    let started = Instant::now();
    let result: AppResult<serde_json::Value> = (|| {
        bootstrap::verify_rootfs(&rootfs, BYTES, HASH)?;
        let mut token = [0u8; 32];
        getrandom::fill(&mut token).map_err(AppError::internal)?;
        let token: String = token.iter().map(|b| format!("{b:02x}")).collect();
        let journal = bootstrap::BootstrapJournal::new(paths.data.clone(), HASH.into(), token)?;
        bootstrap::import(&runner, &paths.state, &journal, &rootfs, BYTES)?;
        // Fresh systemd can take a few moments. Verification is idempotent while
        // journal remains Imported, each command and total runtime are bounded.
        loop {
            match bootstrap::verify_imported(&runner, &paths.state) {
                Ok(_) => break,
                Err(error) if started.elapsed() < Duration::from_secs(90) => {
                    if bootstrap::load(&paths.state)?
                        .is_none_or(|j| j.state != bootstrap::BootstrapState::Imported)
                    {
                        return Err(error);
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(error) => return Err(error),
            }
        }
        let inventory = checked_output(
            &runner,
            &command(&[
                "/usr/bin/dpkg-query",
                "-W",
                "-f=${Package}\\t${Version}\\t${Architecture}\\n",
            ]),
        )?;
        let mut actual: Vec<_> = inventory.lines().map(str::to_owned).collect();
        actual.sort();
        let lock = fs::read_to_string(repo.join("engine/packages.lock.tsv"))?;
        let mut expected: Vec<_> = lock
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(str::to_owned)
            .collect();
        expected.sort();
        if actual != expected || actual.len() != 138 {
            return Err(AppError::invalid(
                "Fixture package inventory differs from the exact 138-package lock.",
            ));
        }
        storage::write_file_atomically(
            &paths.state.join("selected-engine.json"),
            &serde_json::to_vec(&runtime::engine::EngineBinding::managed_wsl())
                .map_err(AppError::internal)?,
        )?;
        storage::write_file_atomically(
            &paths.state.join("engine-selection-configured"),
            b"configured\n",
        )?;
        let vhd_bytes = fs::metadata(paths.data.join("ext4.vhdx"))?.len();
        // This is the same production transaction with only its test-private
        // paths/namespace changed. Consent, locks, inventory and rechecks apply.
        execute_at(&runner, &paths, wsl::DISTRO, true)?;
        if paths.data.exists()
            || paths.state.join(bootstrap::JOURNAL_FILE).exists()
            || paths.state.join(bootstrap::TOKEN_FILE).exists()
            || paths.state.join("selected-engine.json").exists()
        {
            return Err(AppError::invalid(
                "Fixture cleanup left a native identity/disk footprint.",
            ));
        }
        Ok(
            json!({"package_count":actual.len(),"package_inventory_sha256":format!("{:x}",Sha256::digest(inventory.as_bytes())),"fresh_vhd_file_bytes":vhd_bytes,"removed_receipt":serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state.join("removal-v1.json"))?).map_err(AppError::internal)?}),
        )
    })();
    // Always inspect native state and the real full inventory, even on failure.
    // Uncertain import/removal leaves fixture journal/token/marker for review.
    let native_after = native_state().unwrap();
    let inventory_after = real_inventory().unwrap();
    let fixture_absent = !inventory_after
        .iter()
        .any(|n| n.eq_ignore_ascii_case(&fixture));
    let passed = result.is_ok()
        && native_before == native_after
        && inventory_before == inventory_after
        && fixture_absent;
    let record = json!({"schema_version":1,"passed":passed,"proof":"actual_production_removal_transaction_in_unique_test_namespace_on_existing_windows_host","fixture":fixture,"rootfs_bytes":BYTES,"rootfs_sha256":HASH,"engine_removal_source_sha256":format!("{:x}",Sha256::digest(fs::read(repo.join("src/engine_removal.rs")).unwrap())),"fixture_test_sha256":format!("{:x}",Sha256::digest(fs::read(repo.join("src/engine_removal/real_test.rs")).unwrap())),"elapsed_seconds":started.elapsed().as_secs_f64(),"native_identity_files_unchanged":native_before==native_after,"native_identity_hashes":native_before.iter().map(|(n,b)|json!({"file":n,"sha256":b.as_ref().map(|v|format!("{:x}",Sha256::digest(v)))})).collect::<Vec<_>>(),"distro_inventory_before":inventory_before,"distro_inventory_after":inventory_after,"fixture_unregistered":fixture_absent,"command_count":runner.calls.lock().unwrap().len(),"result":result.as_ref().ok(),"error":result.as_ref().err().map(ToString::to_string),"limitations":["Existing Windows/WSL host; not clean-PC/fixed-name bootstrap or native WebView acceptance","Only the test-private adapter scopes fixed-distro commands to a new unique fixture","Production engine and saved app data were never removal targets","No image pulls or app workloads; all-container/all-volume inventories are empty","Marker, bounded receipt and supervisor lock files retained in ignored fixture root; uncertainty is never broad-cleaned"]});
    let receipt_name = if passed {
        "windows-engine-removal-2026-10-04.json".to_owned()
    } else {
        format!("windows-engine-removal-failed-2026-10-04-{fixture}.json")
    };
    let receipt = repo.join("docs/evidence").join(receipt_name);
    // Preserve each failed attempt without replacing an accepted proof. A
    // successful rerun also cannot silently overwrite an earlier receipt.
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&receipt)
        .unwrap();
    use std::io::Write;
    output
        .write_all(&serde_json::to_vec_pretty(&record).unwrap())
        .unwrap();
    output.sync_all().unwrap();
    assert!(passed, "Removal fixture did not pass; retained its exact ignored root and failure receipt for review.");
}
