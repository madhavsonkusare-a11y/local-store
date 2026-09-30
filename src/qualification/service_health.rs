//! All-service qualification: an answering web page cannot hide a dead worker.
use super::*;
use crate::plan::{DeploymentPlan, HealthTest};
use std::collections::BTreeSet;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct Container {
    id: String,
    project: String,
    service: String,
    status: String,
    exit_code: i64,
    health: Option<String>,
    memory_limit: i64,
    nano_cpus: i64,
    pids_limit: Option<i64>,
}

// Only fields needed for the verdict. Never read environment variables,
// health-check output, mounts or credentials into qualification evidence.
const FORMAT: &str = r#"{"id":{{json .Id}},"project":{{json (index .Config.Labels "com.docker.compose.project")}},"service":{{json (index .Config.Labels "com.docker.compose.service")}},"status":{{json .State.Status}},"exit_code":{{.State.ExitCode}},"health":{{with index .State "Health"}}{{json .Status}}{{else}}null{{end}},"memory_limit":{{.HostConfig.Memory}},"nano_cpus":{{.HostConfig.NanoCpus}},"pids_limit":{{json .HostConfig.PidsLimit}}}"#;

fn check(plan: &DeploymentPlan, project: &str, containers: &[Container]) -> Result<(), String> {
    if containers.len() != plan.services.len() {
        return Err(format!(
            "expected {} service containers, found {}",
            plan.services.len(),
            containers.len()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for container in containers {
        if container.project != project || !ids.insert(&container.id) {
            return Err(
                "service inspection returned a different project or duplicate container".into(),
            );
        }
        let service = plan
            .services
            .iter()
            .find(|s| s.name == container.service)
            .ok_or_else(|| "service inspection returned an unexpected service".to_owned())?;
        if !seen.insert(&container.service) {
            return Err(format!("multiple containers for service {}", service.name));
        }
        let limits = &service.overrides;
        if limits
            .memory_limit_bytes
            .is_some_and(|bytes| i64::try_from(bytes).ok() != Some(container.memory_limit))
            || limits
                .cpu_limit_millicores
                .is_some_and(|millicores| i64::from(millicores) * 1_000_000 != container.nano_cpus)
            || limits
                .pids_limit
                .is_some_and(|pids| Some(i64::from(pids)) != container.pids_limit)
        {
            return Err(format!(
                "service {} does not enforce its reviewed resource limits",
                service.name
            ));
        }
        if plan.is_job(&service.name) {
            if container.status != "exited" || container.exit_code != 0 {
                return Err(format!(
                    "job {} has not completed successfully",
                    service.name
                ));
            }
        } else {
            if container.status != "running" {
                return Err(format!("service {} is not running", service.name));
            }
            let declared = service
                .overrides
                .healthcheck
                .as_ref()
                .and_then(|h| h.test.as_ref())
                .is_some_and(|test| !matches!(test, HealthTest::Disabled));
            if declared && container.health.is_none() {
                return Err(format!(
                    "service {} is missing its declared health status",
                    service.name
                ));
            }
            // Also enforce checks inherited from the image.
            if container
                .health
                .as_deref()
                .is_some_and(|state| state != "healthy")
            {
                return Err(format!("service {} is not healthy", service.name));
            }
        }
    }
    Ok(())
}

fn snapshot(
    runner: &dyn ProcessRunner,
    plan: &DeploymentPlan,
    project: &str,
    timeout: Duration,
) -> Result<(), String> {
    let started = Instant::now();
    let listed = runner
        .run(&CommandSpec::new(
            "docker",
            vec![
                "container".into(),
                "ls".into(),
                "-aq".into(),
                "--filter".into(),
                format!("label=com.docker.compose.project={project}"),
            ],
            None,
            timeout,
        ))
        .map_err(|_| "service inventory could not be read".to_owned())?;
    if !listed.success || listed.truncated {
        return Err("service inventory failed or was truncated".into());
    }
    let ids: Vec<_> = listed.stdout.split_whitespace().collect();
    if ids.len() != plan.services.len() {
        return Err(format!(
            "expected {} service containers, found {}",
            plan.services.len(),
            ids.len()
        ));
    }
    if ids
        .iter()
        .any(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("service inventory returned an invalid container ID".into());
    }
    let remaining = timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err("service inventory exceeded its deadline".into());
    }
    let mut args = vec![
        "container".into(),
        "inspect".into(),
        "--format".into(),
        FORMAT.into(),
    ];
    args.extend(ids.into_iter().map(str::to_owned));
    let output = runner
        .run(&CommandSpec::new("docker", args, None, remaining))
        .map_err(|_| "service state could not be inspected".to_owned())?;
    if !output.success || output.truncated {
        return Err("service state inspection failed or was truncated".into());
    }
    let containers = output
        .stdout
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Container>, _>>()
        .map_err(|_| "service state inspection returned invalid JSON".to_owned())?;
    check(plan, project, &containers)
}

pub(super) fn wait(
    runner: &dyn ProcessRunner,
    plan: &DeploymentPlan,
    project: &str,
    timeout: Duration,
) -> Result<(), String> {
    let started = Instant::now();
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err("all-service health deadline expired".into());
        }
        match snapshot(runner, plan, project, remaining.min(DIAGNOSTIC_TIMEOUT)) {
            Ok(()) => return Ok(()),
            Err(reason) => {
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining <= Duration::from_millis(250) {
                    return Err(reason);
                }
                std::thread::sleep(remaining.min(Duration::from_millis(250)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> DeploymentPlan {
        crate::offerings::offering("docmost")
            .unwrap()
            .plan_template(None)
            .unwrap()
            .plan
    }

    fn running(plan: &DeploymentPlan) -> Vec<Container> {
        plan.services
            .iter()
            .enumerate()
            .map(|(i, s)| Container {
                id: format!("{i:064x}"),
                project: "owned".into(),
                service: s.name.clone(),
                status: "running".into(),
                exit_code: 0,
                health: Some("healthy".into()),
                memory_limit: 0,
                nano_cpus: 0,
                pids_limit: None,
            })
            .collect()
    }

    #[test]
    fn a_healthy_web_service_cannot_hide_a_dead_or_unhealthy_dependency() {
        let plan = plan();
        let mut containers = running(&plan);
        assert!(containers.len() > 1);
        assert!(check(&plan, "owned", &containers).is_ok());
        containers[1].status = "exited".into();
        assert!(check(&plan, "owned", &containers)
            .unwrap_err()
            .contains("not running"));
        containers[1].status = "running".into();
        containers[1].health = Some("unhealthy".into());
        assert!(check(&plan, "owned", &containers)
            .unwrap_err()
            .contains("not healthy"));
    }

    #[test]
    fn missing_duplicate_and_foreign_services_fail() {
        let plan = plan();
        let mut containers = running(&plan);
        containers.pop();
        assert!(check(&plan, "owned", &containers).is_err());
        let mut containers = running(&plan);
        containers[1].service = containers[0].service.clone();
        assert!(check(&plan, "owned", &containers).is_err());
        let mut containers = running(&plan);
        containers[0].project = "bystander".into();
        assert!(check(&plan, "owned", &containers).is_err());
    }

    #[test]
    fn missing_or_changed_docker_resource_limits_fail_health() {
        let mut plan = plan();
        let service = &mut plan.services[0];
        service.overrides.memory_limit_bytes = Some(512 * 1024 * 1024);
        service.overrides.cpu_limit_millicores = Some(2000);
        service.overrides.pids_limit = Some(512);
        let mut containers = running(&plan);
        assert!(check(&plan, "owned", &containers).is_err());
        containers[0].memory_limit = 512 * 1024 * 1024;
        containers[0].nano_cpus = 2_000_000_000;
        containers[0].pids_limit = Some(512);
        assert!(check(&plan, "owned", &containers).is_ok());
        containers[0].nano_cpus = 1_000_000_000;
        assert!(check(&plan, "owned", &containers).is_err());
    }

    #[test]
    fn jobs_must_exit_zero() {
        let mut plan = plan();
        let job = plan.services[1].name.clone();
        plan.services[0]
            .overrides
            .completed_dependencies
            .insert(job);
        let mut containers = running(&plan);
        assert!(check(&plan, "owned", &containers).is_err());
        containers[1].status = "exited".into();
        assert!(check(&plan, "owned", &containers).is_ok());
        containers[1].exit_code = 1;
        assert!(check(&plan, "owned", &containers).is_err());
    }

    #[test]
    fn declared_health_must_be_present_and_image_health_must_be_ready() {
        let mut plan = plan();
        plan.services[0].overrides.healthcheck = Some(crate::plan::PlanHealthcheck {
            test: Some(HealthTest::Exec(vec!["true".into()])),
            ..Default::default()
        });
        let mut containers = running(&plan);
        containers[0].health = None;
        assert!(check(&plan, "owned", &containers)
            .unwrap_err()
            .contains("missing"));
        containers[0].health = Some("starting".into());
        assert!(check(&plan, "owned", &containers).is_err());
    }

    #[test]
    #[ignore = "real Docker; set LOCAL_STORE_HEALTH_TEST_IMAGE to an existing image with sleep"]
    fn real_docker_inspection_detects_a_stopped_worker() {
        let image =
            std::env::var("LOCAL_STORE_HEALTH_TEST_IMAGE").expect("explicit local fixture image");
        let runner = crate::runtime::SystemProcessRunner;
        let project = format!(
            "local-store-health-proof-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut plan = plan();
        for service in &mut plan.services {
            service.overrides = Default::default();
        }
        let bystanders = Bystanders::note(&runner).unwrap();
        {
            let owned = OwnedResources::new(&runner, &project);
            let mut ids = Vec::new();
            for service in &plan.services {
                let output = runner
                    .run(&CommandSpec::new(
                        "docker",
                        vec![
                            "run".into(),
                            "-d".into(),
                            "--network".into(),
                            "none".into(),
                            "--label".into(),
                            format!("com.docker.compose.project={project}"),
                            "--label".into(),
                            format!("com.docker.compose.service={}", service.name),
                            image.clone(),
                            "sleep".into(),
                            "120".into(),
                        ],
                        None,
                        DIAGNOSTIC_TIMEOUT,
                    ))
                    .unwrap();
                assert!(output.success);
                ids.push(output.stdout.trim().to_owned());
            }
            snapshot(&runner, &plan, &project, DIAGNOSTIC_TIMEOUT).unwrap();
            let stopped = runner
                .run(&CommandSpec::new(
                    "docker",
                    vec!["stop".into(), "--time".into(), "1".into(), ids[1].clone()],
                    None,
                    DIAGNOSTIC_TIMEOUT,
                ))
                .unwrap();
            assert!(stopped.success);
            assert!(snapshot(&runner, &plan, &project, DIAGNOSTIC_TIMEOUT)
                .unwrap_err()
                .contains("not running"));
            drop(owned);
        }
        OwnedResources::new(&runner, &project)
            .all_removed()
            .unwrap();
        bystanders.survived(&runner).unwrap();
    }
}
