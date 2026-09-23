//! Bounded, project-scoped resource sampling for qualification evidence.
use super::*;
use std::path::Path;

const STATS_FORMAT: &str = "{{.Name}}\t{{.MemUsage}}";

fn bytes(value: &str) -> Result<u64, String> {
    let value = value.trim();
    let split = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .ok_or_else(|| "memory usage omitted its unit".to_owned())?;
    let (number, unit) = value.split_at(split);
    let scale = match unit {
        "B" => 1,
        "kB" => 1_000,
        "MB" => 1_000_000,
        "GB" => 1_000_000_000,
        "TB" => 1_000_000_000_000,
        "KiB" => 1_024,
        "MiB" => 1_048_576,
        "GiB" => 1_073_741_824,
        "TiB" => 1_099_511_627_776,
        _ => return Err("memory usage used an unsupported unit".into()),
    };
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (number.contains('.') && fraction.is_empty())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
    {
        return Err("memory usage was not a bounded decimal".into());
    }
    let whole: u64 = whole
        .parse()
        .map_err(|_| "memory usage was too large".to_owned())?;
    let fraction_value: u64 = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse()
            .map_err(|_| "memory usage was not a decimal".to_owned())?
    };
    let denominator = 10_u64.pow(fraction.len() as u32);
    whole
        .checked_mul(scale)
        .and_then(|base| {
            fraction_value
                .checked_mul(scale)
                .map(|part| base.saturating_add(part / denominator))
        })
        .ok_or_else(|| "memory usage overflowed".to_owned())
}

pub(super) struct Snapshot {
    pub memory_bytes: BTreeMap<String, u64>,
    pub managed_storage_bytes: u64,
    pub named_volume_bytes: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VolumeInspection {
    name: String,
    driver: String,
    scope: String,
    mountpoint: String,
    labels: BTreeMap<String, String>,
}

fn named_volume_bytes(
    runner: &dyn ProcessRunner,
    project: &str,
    names: &[String],
) -> Result<BTreeMap<String, u64>, String> {
    if names.is_empty() {
        return Ok(BTreeMap::new());
    }
    if names.len() > 16 {
        return Err("too many named volumes to measure safely".into());
    }
    let managed = runner
        .engine_binding()
        .map_err(|_| "selected engine could not be identified".to_owned())?
        .is_some_and(|binding| binding.is_wsl());
    if !managed {
        return Err("named volume disk usage requires the managed WSL engine".into());
    }
    let expected: BTreeMap<_, _> = names
        .iter()
        .map(|name| (format!("{project}_{name}"), name.as_str()))
        .collect();
    if expected.len() != names.len() {
        return Err("named volume declarations were duplicated".into());
    }
    let mut args = vec![
        "volume".into(),
        "inspect".into(),
        "--format".into(),
        "{{json .}}".into(),
    ];
    args.extend(expected.keys().cloned());
    let inspected = runner
        .run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT))
        .map_err(|_| "named volume inventory could not be read".to_owned())?;
    if !inspected.success || inspected.truncated {
        return Err("named volume inventory failed or was truncated".into());
    }
    let mut mountpoints = BTreeMap::new();
    for line in inspected.stdout.lines() {
        let volume: VolumeInspection = serde_json::from_str(line)
            .map_err(|_| "named volume inventory returned invalid JSON".to_owned())?;
        let declared = expected
            .get(&volume.name)
            .ok_or_else(|| "named volume inventory returned a foreign volume".to_owned())?;
        if volume.driver != "local"
            || volume.scope != "local"
            || volume
                .labels
                .get("com.docker.compose.project")
                .map(String::as_str)
                != Some(project)
            || volume
                .labels
                .get("com.docker.compose.volume")
                .map(String::as_str)
                != Some(*declared)
            || !volume.mountpoint.starts_with('/')
            || volume.mountpoint == "/"
            || volume
                .mountpoint
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || part == "." || part == "..")
            || volume.mountpoint.chars().any(char::is_control)
            || mountpoints
                .insert((*declared).to_owned(), volume.mountpoint)
                .is_some()
        {
            return Err("named volume inventory failed ownership or path checks".into());
        }
    }
    if mountpoints.len() != expected.len() {
        return Err("named volume inventory omitted a declared volume".into());
    }
    let mut sizes = BTreeMap::new();
    for (name, mountpoint) in mountpoints {
        let mut command = CommandSpec::new(
            "wsl.exe",
            vec![
                "--distribution".into(),
                crate::runtime::engine::wsl::DISTRO.into(),
                "--user".into(),
                "root".into(),
                "--cd".into(),
                "/".into(),
                "--exec".into(),
                "/usr/bin/du".into(),
                "--bytes".into(),
                "--summarize".into(),
                "--one-file-system".into(),
                "--".into(),
                mountpoint.clone(),
            ],
            None,
            DIAGNOSTIC_TIMEOUT,
        );
        command.remove_env.push("WSLENV".into());
        let measured = runner
            .run(&command)
            .map_err(|_| "named volume disk usage could not be read".to_owned())?;
        if !measured.success || measured.truncated {
            return Err("named volume disk usage failed or was truncated".into());
        }
        let (value, path) = measured
            .stdout
            .trim_end_matches('\n')
            .split_once('\t')
            .ok_or_else(|| "named volume disk usage returned an invalid row".to_owned())?;
        if path != mountpoint || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("named volume disk usage returned an unexpected path or size".into());
        }
        sizes.insert(
            name,
            value
                .parse::<u64>()
                .map_err(|_| "named volume disk usage overflowed".to_owned())?,
        );
    }
    Ok(sizes)
}

fn directory_bytes(root: &Path) -> Result<u64, String> {
    const MAX_ENTRIES: usize = 1_000_000;
    let mut pending = vec![root.to_path_buf()];
    let mut entries = 0_usize;
    let mut total = 0_u64;
    while let Some(directory) = pending.pop() {
        let children = std::fs::read_dir(&directory)
            .map_err(|_| "managed storage could not be read".to_owned())?;
        for child in children {
            let child = child.map_err(|_| "managed storage could not be read".to_owned())?;
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err("managed storage had too many entries to measure safely".into());
            }
            let metadata = child
                .path()
                .symlink_metadata()
                .map_err(|_| "managed storage metadata could not be read".to_owned())?;
            if metadata.file_type().is_symlink() {
                return Err("managed storage contained a symbolic link".into());
            }
            if metadata.is_dir() {
                pending.push(child.path());
            } else if metadata.is_file() {
                total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| "managed storage size overflowed".to_owned())?;
            }
        }
    }
    Ok(total)
}

pub(super) fn snapshot(
    runner: &dyn ProcessRunner,
    project: &str,
    project_dir: &Path,
    named_volumes: &[String],
) -> Result<Snapshot, String> {
    let listed = runner
        .run(&CommandSpec::new(
            "docker",
            vec![
                "container".into(),
                "ls".into(),
                "-q".into(),
                "--filter".into(),
                format!("label=com.docker.compose.project={project}"),
            ],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .map_err(|_| "running container inventory could not be read".to_owned())?;
    if !listed.success || listed.truncated {
        return Err("running container inventory failed or was truncated".into());
    }
    let ids: Vec<_> = listed.stdout.split_whitespace().collect();
    if ids.is_empty()
        || ids
            .iter()
            .any(|id| !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err("running container inventory was empty or invalid".into());
    }
    let mut args = vec![
        "stats".into(),
        "--no-stream".into(),
        "--format".into(),
        STATS_FORMAT.into(),
    ];
    args.extend(ids.iter().map(|id| (*id).to_owned()));
    let output = runner
        .run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT))
        .map_err(|_| "container memory usage could not be read".to_owned())?;
    if !output.success || output.truncated {
        return Err("container memory usage failed or was truncated".into());
    }
    let mut sample = BTreeMap::new();
    for line in output.stdout.lines() {
        let (name, usage) = line
            .split_once('\t')
            .ok_or_else(|| "container memory usage had an invalid row".to_owned())?;
        if name.is_empty()
            || name.len() > 256
            || name.chars().any(char::is_control)
            || sample.contains_key(name)
        {
            return Err("container memory usage had an invalid name".into());
        }
        let used = usage.split_once('/').map(|(used, _)| used).unwrap_or(usage);
        sample.insert(name.to_owned(), bytes(used)?);
    }
    if sample.len() != ids.len() {
        return Err("container memory usage omitted or duplicated a container".into());
    }
    Ok(Snapshot {
        memory_bytes: sample,
        managed_storage_bytes: directory_bytes(project_dir)?,
        named_volume_bytes: named_volume_bytes(runner, project, named_volumes)?,
    })
}

pub(super) fn image_sizes(
    runner: &dyn ProcessRunner,
    image_ids: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, u64>, String> {
    let ids: std::collections::BTreeSet<_> = image_ids.values().cloned().collect();
    if ids.is_empty()
        || ids.iter().any(|id| {
            !id.strip_prefix("sha256:").is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        })
    {
        return Err("resolved image ids were empty or invalid".into());
    }
    let mut args = vec![
        "image".into(),
        "inspect".into(),
        "--format".into(),
        "{{.Id}}\t{{.Size}}".into(),
    ];
    args.extend(ids.iter().cloned());
    let output = runner
        .run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT))
        .map_err(|_| "resolved image sizes could not be read".to_owned())?;
    if !output.success || output.truncated {
        return Err("resolved image sizes failed or were truncated".into());
    }
    let mut sizes = BTreeMap::new();
    for line in output.stdout.lines() {
        let (id, size) = line
            .split_once('\t')
            .ok_or_else(|| "resolved image sizes had an invalid row".to_owned())?;
        if !ids.contains(id) || sizes.contains_key(id) {
            return Err("resolved image sizes named an unexpected image".into());
        }
        let size = size
            .parse::<u64>()
            .map_err(|_| "resolved image size was not an integer".to_owned())?;
        sizes.insert(id.to_owned(), size);
    }
    if sizes.len() != ids.len() {
        return Err("resolved image sizes omitted or duplicated an image".into());
    }
    Ok(sizes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{CancelToken, ProcessError, ProcessOutput};
    use std::{collections::VecDeque, sync::Mutex};

    struct Sequence(Mutex<VecDeque<ProcessOutput>>);
    impl ProcessRunner for Sequence {
        fn run_cancellable(
            &self,
            _: &CommandSpec,
            _: &CancelToken,
        ) -> Result<ProcessOutput, ProcessError> {
            Ok(self.0.lock().unwrap().pop_front().unwrap())
        }
    }
    fn output(stdout: &str) -> ProcessOutput {
        ProcessOutput {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
            truncated: false,
        }
    }

    #[test]
    fn docker_memory_units_become_stable_integer_bytes() {
        assert_eq!(bytes("0B").unwrap(), 0);
        assert_eq!(bytes("1.5KiB").unwrap(), 1_536);
        assert_eq!(bytes("12.25MiB").unwrap(), 12_845_056);
        assert_eq!(bytes("2GB").unwrap(), 2_000_000_000);
        for invalid in ["", "12", "12.B", "NaNMiB", "1.1234567MiB", "1XB"] {
            assert!(bytes(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn a_snapshot_is_scoped_and_refuses_missing_rows() {
        let root = std::env::temp_dir().join(format!(
            "local-store-resource-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/item"), b"12345").unwrap();
        let runner = Sequence(Mutex::new(VecDeque::from([
            output("abc123\ndef456\n"),
            output("owned-web-1\t12.25MiB / 1GiB\nowned-db-1\t1.5KiB / 1GiB\n"),
        ])));
        let sample = snapshot(&runner, "owned", &root, &[]).unwrap();
        assert_eq!(sample.memory_bytes["owned-web-1"], 12_845_056);
        assert_eq!(sample.memory_bytes["owned-db-1"], 1_536);
        assert_eq!(sample.managed_storage_bytes, 5);
        assert_eq!(sample.named_volume_bytes, BTreeMap::new());
        assert!(named_volume_bytes(&runner, "owned", &["data".into()]).is_err());

        let missing = Sequence(Mutex::new(VecDeque::from([
            output("abc123\ndef456\n"),
            output("owned-web-1\t1MiB / 1GiB\n"),
        ])));
        assert!(snapshot(&missing, "owned", &root, &[]).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn image_sizes_are_deduplicated_and_exact() {
        let a = format!("sha256:{}", "a".repeat(64));
        let b = format!("sha256:{}", "b".repeat(64));
        let runner = Sequence(Mutex::new(VecDeque::from([output(&format!(
            "{a}\t123\n{b}\t456\n"
        ))])));
        let sizes = image_sizes(
            &runner,
            &[
                ("web".into(), a.clone()),
                ("worker".into(), a.clone()),
                ("db".into(), b.clone()),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap();
        assert_eq!(sizes.len(), 2);
        assert_eq!(sizes[&a], 123);
        assert_eq!(sizes[&b], 456);
    }

    #[test]
    fn managed_volume_measurement_requires_compose_ownership_and_exact_du_path() {
        struct Managed {
            outputs: Mutex<VecDeque<ProcessOutput>>,
            calls: Mutex<Vec<CommandSpec>>,
        }
        impl ProcessRunner for Managed {
            fn engine_binding(
                &self,
            ) -> crate::error::AppResult<Option<crate::runtime::engine::EngineBinding>>
            {
                Ok(Some(crate::runtime::engine::EngineBinding::managed_wsl()))
            }
            fn run_cancellable(
                &self,
                spec: &CommandSpec,
                _: &CancelToken,
            ) -> Result<ProcessOutput, ProcessError> {
                self.calls.lock().unwrap().push(spec.clone());
                Ok(self.outputs.lock().unwrap().pop_front().unwrap())
            }
        }
        let mountpoint = "/var/lib/docker/volumes/owned_data/_data";
        let volume = format!(
            "{{\"Name\":\"owned_data\",\"Driver\":\"local\",\"Scope\":\"local\",\"Mountpoint\":\"{mountpoint}\",\"Labels\":{{\"com.docker.compose.project\":\"owned\",\"com.docker.compose.volume\":\"data\"}}}}\n"
        );
        let runner = Managed {
            outputs: Mutex::new(VecDeque::from([
                output(&volume),
                output(&format!("123\t{mountpoint}\n")),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        let measured = named_volume_bytes(&runner, "owned", &["data".into()]).unwrap();
        assert_eq!(measured["data"], 123);
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].program, "docker");
        assert_eq!(calls[1].program, "wsl.exe");
        assert!(calls[1].args.contains(&"/usr/bin/du".into()));
        assert!(calls[1].remove_env.contains(&"WSLENV".into()));
        drop(calls);

        let foreign = Managed {
            outputs: Mutex::new(VecDeque::from([output(&volume.replace(
                "\"com.docker.compose.project\":\"owned\"",
                "\"com.docker.compose.project\":\"foreign\"",
            ))])),
            calls: Mutex::new(Vec::new()),
        };
        assert!(named_volume_bytes(&foreign, "owned", &["data".into()]).is_err());
        assert_eq!(foreign.calls.lock().unwrap().len(), 1);
    }
}
