//! Bounded, project-scoped memory sampling for qualification evidence.
use super::*;

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

pub(super) fn snapshot(
    runner: &dyn ProcessRunner,
    project: &str,
) -> Result<BTreeMap<String, u64>, String> {
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
    Ok(sample)
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
        let runner = Sequence(Mutex::new(VecDeque::from([
            output("abc123\ndef456\n"),
            output("owned-web-1\t12.25MiB / 1GiB\nowned-db-1\t1.5KiB / 1GiB\n"),
        ])));
        let sample = snapshot(&runner, "owned").unwrap();
        assert_eq!(sample["owned-web-1"], 12_845_056);
        assert_eq!(sample["owned-db-1"], 1_536);

        let missing = Sequence(Mutex::new(VecDeque::from([
            output("abc123\ndef456\n"),
            output("owned-web-1\t1MiB / 1GiB\n"),
        ])));
        assert!(snapshot(&missing, "owned").is_err());
    }
}
