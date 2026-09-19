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
        let sample = snapshot(&runner, "owned", &root).unwrap();
        assert_eq!(sample.memory_bytes["owned-web-1"], 12_845_056);
        assert_eq!(sample.memory_bytes["owned-db-1"], 1_536);
        assert_eq!(sample.managed_storage_bytes, 5);

        let missing = Sequence(Mutex::new(VecDeque::from([
            output("abc123\ndef456\n"),
            output("owned-web-1\t1MiB / 1GiB\n"),
        ])));
        assert!(snapshot(&missing, "owned", &root).is_err());
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
}
