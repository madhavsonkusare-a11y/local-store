//! One way to ask whether an app actually works for a person.
//!
//! The product promise is narrow and testable: after Docker is set up, a
//! supported app installs with one action, opens to a useful screen, and keeps
//! its data through a restart and a reinstall. Answering that for one app was
//! never the hard part — `privatebin`, `nodered` and `flatnotes` each got a
//! bespoke test, and each carried its own copy of the same scaffolding: a
//! Docker helper, a cleanup guard, a private config root, a hand-written
//! evidence file. Three copies is where a fourth app stops being worth it.
//!
//! This is that scaffolding, once. An app supplies only what is specific to
//! it — how to use it in a browser — and the harness supplies everything else:
//! isolation, bounded steps, cleanup that runs even when a step fails, proof
//! that containers belonging to anything else survived, and a machine-readable
//! result that is derived from what happened rather than written by hand.
//!
//! Nothing here decides whether an app is offered. It produces evidence; a
//! review reads it, and a person approves it.
use crate::error::{AppError, AppResult};
use crate::runtime::{CommandSpec, HealthProbe, ProcessRunner, DIAGNOSTIC_TIMEOUT};
use crate::setup::PlanTemplate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod resource_usage;
mod service_health;

/// Where in an app's life a first-use check is being run.
///
/// The same check runs at all three points on purpose: passing once proves the
/// app works, passing after a restart and a reinstall proves the person's data
/// and credentials survived — which is the half a health probe cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    FirstInstall,
    AfterRestart,
    AfterReinstall,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Phase::FirstInstall => "first install",
            Phase::AfterRestart => "after restart",
            Phase::AfterReinstall => "after a keep-data reinstall",
        }
    }
}

/// The part of qualification only the app itself can define.
///
/// Everything else is the same for every app. This is deliberately the whole
/// interface: an app that needs more than "use it at this address" is telling
/// you something about the app, not about the harness.
pub trait FirstUse {
    /// What this check proves, in the words the evidence will use.
    fn describes(&self) -> &str;

    /// Exercise the app. `Ok(())` means a person could use it; `Err` says why
    /// not, in text safe to record.
    fn exercise(&self, phase: Phase, address: &str) -> Result<(), String>;
}

/// A first-use check that never fails, for an app whose only claim is that it
/// answers. Being explicit beats an `Option` that quietly proves less.
pub struct AnswersOnly;

impl FirstUse for AnswersOnly {
    fn describes(&self) -> &str {
        "the app answers on its address (no first-use check was supplied)"
    }
    fn exercise(&self, _: Phase, _: &str) -> Result<(), String> {
        Ok(())
    }
}

/// A first-use check implemented as a Node script, which is what all three of
/// the existing app probes already are.
pub struct ScriptProbe {
    pub script: PathBuf,
    pub describes: String,
    /// Extra arguments after the phase and the address, for a probe that needs
    /// to know a username or a state file.
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl ScriptProbe {
    pub fn new(script: impl Into<PathBuf>, describes: impl Into<String>) -> Self {
        Self {
            script: script.into(),
            describes: describes.into(),
            args: Vec::new(),
            timeout: Duration::from_secs(150),
        }
    }
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

/// The line a failed probe actually said, out of everything it printed.
///
/// Node prints an uncaught error's message near the top, then the stack, then
/// its own version last — so the tail of stderr, which is what was kept, is
/// the one part that says nothing. WordPress's first failure was recorded as
/// "Node.js v24.14.0 } name: 'Error'".
fn probe_failure(stderr: &str) -> String {
    let thrown = stderr.lines().map(str::trim).find(|line| {
        line.split_once(": ").is_some_and(|(kind, _)| {
            kind.ends_with("Error") && kind.chars().all(|c| c.is_ascii_alphanumeric())
        })
    });
    if let Some(line) = thrown {
        return line.to_owned();
    }
    let mut tail: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("Node.js v"))
        .rev()
        .take(4)
        .collect();
    tail.reverse();
    tail.join(" ")
}

impl FirstUse for ScriptProbe {
    fn describes(&self) -> &str {
        &self.describes
    }
    fn exercise(&self, phase: Phase, address: &str) -> Result<(), String> {
        let mut args = vec![
            self.script.to_string_lossy().into_owned(),
            match phase {
                Phase::FirstInstall => "first-use".to_owned(),
                _ => "verify".to_owned(),
            },
            address.to_owned(),
        ];
        args.extend(self.args.iter().cloned());
        let spec = CommandSpec::new(
            "node",
            args,
            Some(
                self.script
                    .parent()
                    .and_then(Path::parent)
                    .unwrap_or(Path::new("."))
                    .to_path_buf(),
            ),
            self.timeout,
        );
        let output = crate::runtime::SystemProcessRunner
            .run(&spec)
            .map_err(|error| crate::runtime::redact(&error.message))?;
        if output.success {
            Ok(())
        } else {
            // Whatever a probe prints could contain an answer somebody typed.
            Err(crate::runtime::redact(&probe_failure(&output.stderr)))
        }
    }
}

/// One thing the harness checked, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
    pub step: String,
    pub passed: bool,
    /// Why it failed, redacted. Absent when it passed, so a passing report
    /// cannot accidentally carry output nobody read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// What a qualification run concluded.
///
/// Runtime duration stays out of evidence, while `recorded_at_unix` provides
/// the bounded freshness needed to decide whether a pass may be reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// Version 0 is historical unversioned evidence. Version 1 adds mandatory
    /// all-service checks; neither version certifies the future managed engine.
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub recorded_at_unix: u64,
    pub app: String,
    pub passed: bool,
    pub scope: String,
    pub promotion: String,
    pub source_revision: String,
    pub images: Vec<String>,
    /// Resolved image ids, so the evidence names what actually ran rather than
    /// a tag that may since have moved.
    pub image_ids: BTreeMap<String, String>,
    pub first_use: String,
    /// Runtime measurements from the same lifecycle run. Older evidence stays
    /// readable but cannot satisfy the stronger resource-aware gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurements: Option<ResourceMeasurements>,
    pub steps: Vec<StepResult>,
    /// Immutable inputs and runtime facts that determine what this pass proves.
    #[serde(default)]
    pub identity: Option<EvidenceIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMeasurements {
    pub first_start_millis: u64,
    pub samples: u32,
    pub idle_memory_bytes: BTreeMap<String, u64>,
    pub peak_memory_bytes: BTreeMap<String, u64>,
    pub peak_total_memory_bytes: u64,
}

impl ResourceMeasurements {
    fn new(first_start_millis: u64) -> Self {
        Self {
            first_start_millis,
            samples: 0,
            idle_memory_bytes: BTreeMap::new(),
            peak_memory_bytes: BTreeMap::new(),
            peak_total_memory_bytes: 0,
        }
    }

    fn observe(&mut self, sample: BTreeMap<String, u64>) {
        if self.samples == 0 {
            self.idle_memory_bytes = sample.clone();
        }
        let total = sample
            .values()
            .try_fold(0_u64, |total, bytes| total.checked_add(*bytes))
            .unwrap_or(u64::MAX);
        self.peak_total_memory_bytes = self.peak_total_memory_bytes.max(total);
        for (container, bytes) in sample {
            self.peak_memory_bytes
                .entry(container)
                .and_modify(|peak| *peak = (*peak).max(bytes))
                .or_insert(bytes);
        }
        self.samples = self.samples.saturating_add(1);
    }

    fn reusable(&self) -> bool {
        self.first_start_millis > 0
            && self.samples >= 3
            && !self.idle_memory_bytes.is_empty()
            && self
                .idle_memory_bytes
                .keys()
                .all(|name| self.peak_memory_bytes.contains_key(name))
            && self.peak_total_memory_bytes
                >= self.peak_memory_bytes.values().copied().max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIdentity {
    pub source_kind: String,
    pub source_adapter: String,
    pub source_locator: String,
    pub source_revision: String,
    pub source_observed_on: String,
    pub images_observed_on: String,
    pub plan_sha256: String,
    pub requested_images: Vec<String>,
    pub resolved_image_ids: BTreeMap<String, String>,
    pub host_os: String,
    pub host_arch: String,
    pub engine: crate::runtime::engine::EngineBinding,
    pub compose_version: String,
    pub probe_sha256: String,
    pub level: String,
}

impl Evidence {
    pub const MAX_REUSE_AGE_SECS: u64 = 30 * 24 * 60 * 60;
    pub const MAX_SOURCE_AGE_DAYS: i64 = 90;
    pub const MAX_IMAGE_AGE_DAYS: i64 = 30;
    const MAX_FUTURE_SKEW_SECS: u64 = 24 * 60 * 60;

    /// The failing step, if any. A run stops at the first failure, so this is
    /// the reason the whole thing failed.
    pub fn failure(&self) -> Option<&StepResult> {
        self.steps.iter().find(|step| !step.passed)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("evidence is plain data")
    }

    pub fn is_current(&self, identity: &EvidenceIdentity) -> bool {
        self.is_current_at(identity, unix_now())
    }

    pub fn is_current_at(&self, identity: &EvidenceIdentity, now_unix: u64) -> bool {
        self.schema_version == 2
            && self.identity.as_ref() == Some(identity)
            && self
                .measurements
                .as_ref()
                .is_some_and(ResourceMeasurements::reusable)
            && self.recorded_at_unix > 0
            && self.recorded_at_unix <= now_unix.saturating_add(Self::MAX_FUTURE_SKEW_SECS)
            && now_unix.saturating_sub(self.recorded_at_unix) <= Self::MAX_REUSE_AGE_SECS
            && observation_is_fresh(
                &identity.source_observed_on,
                now_unix,
                Self::MAX_SOURCE_AGE_DAYS,
            )
            && observation_is_fresh(
                &identity.images_observed_on,
                now_unix,
                Self::MAX_IMAGE_AGE_DAYS,
            )
    }
}

fn observation_is_fresh(date: &str, now_unix: u64, max_age_days: i64) -> bool {
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let Ok(year) = parts[0].parse::<i64>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<i64>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<i64>() else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day < 1
        || day > month_days[(month - 1) as usize]
    {
        return false;
    }
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let observed = era * 146_097 + day_of_era - 719_468;
    let today = (now_unix / 86_400) as i64;
    observed <= today + 1 && today.saturating_sub(observed) <= max_age_days
}

#[cfg(test)]
fn date_for_unix(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Records steps in order and refuses to keep going once one has failed.
#[derive(Default)]
pub struct Steps {
    results: Vec<StepResult>,
}

impl Steps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run a step unless an earlier one already failed.
    ///
    /// Short-circuiting matters for honesty as much as for time: checking
    /// "the data survived a restart" after the restart failed would record a
    /// pass or a failure that means nothing either way.
    pub fn run<T>(
        &mut self,
        name: impl Into<String>,
        body: impl FnOnce() -> Result<T, String>,
    ) -> Option<T> {
        let name = name.into();
        if self.failed() {
            return None;
        }
        match body() {
            Ok(value) => {
                self.results.push(StepResult {
                    step: name,
                    passed: true,
                    detail: None,
                });
                Some(value)
            }
            Err(reason) => {
                self.results.push(StepResult {
                    step: name,
                    passed: false,
                    // Everything recorded goes through redaction, because a
                    // failure is exactly when a container prints its
                    // configuration.
                    detail: Some(crate::runtime::redact(&reason)),
                });
                None
            }
        }
    }

    pub fn failed(&self) -> bool {
        self.results.iter().any(|step| !step.passed)
    }

    pub fn into_results(self) -> Vec<StepResult> {
        self.results
    }
}

/// Containers that existed before a run, so the run can prove it left them
/// alone. Every qualification touches real Docker on a real machine, and the
/// one unacceptable outcome is removing something that was not ours.
pub struct Bystanders {
    before: Vec<String>,
}

impl Bystanders {
    pub fn note(runner: &dyn ProcessRunner) -> AppResult<Self> {
        Ok(Self {
            before: all_containers(runner)?,
        })
    }

    /// Every container that existed before still exists.
    pub fn survived(&self, runner: &dyn ProcessRunner) -> Result<(), String> {
        let after = all_containers(runner).map_err(|error| error.message)?;
        let lost: Vec<&String> = self
            .before
            .iter()
            .filter(|id| !after.contains(id))
            .collect();
        if lost.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} container(s) that belonged to something else were removed",
                lost.len()
            ))
        }
    }
}

fn all_containers(runner: &dyn ProcessRunner) -> AppResult<Vec<String>> {
    let output = runner
        .run(&CommandSpec::new(
            "docker",
            vec!["container".into(), "ls".into(), "-aq".into()],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .map_err(AppError::from)?;
    if !output.success || output.truncated {
        return Err(AppError::invalid("Docker container inspection failed."));
    }
    Ok(output
        .stdout
        .split_whitespace()
        .map(str::to_owned)
        .collect())
}

/// Everything a run created, removed when the run ends however it ends.
///
/// A qualification that fails half way through is the case that matters: it is
/// the one that leaves containers behind, and the one most likely to happen on
/// somebody else's machine.
pub struct OwnedResources<'a> {
    runner: &'a dyn ProcessRunner,
    project: String,
}

impl<'a> OwnedResources<'a> {
    pub fn new(runner: &'a dyn ProcessRunner, project: impl Into<String>) -> Self {
        Self {
            runner,
            project: project.into(),
        }
    }

    /// What this project still owns, by resource kind.
    pub fn remaining(&self) -> Result<BTreeMap<String, usize>, String> {
        let mut counts = BTreeMap::new();
        for (kind, list) in [("container", "-aq"), ("network", "-q"), ("volume", "-q")] {
            counts.insert(kind.to_owned(), self.ids(kind, list)?.len());
        }
        Ok(counts)
    }

    fn ids(&self, kind: &str, list: &str) -> Result<Vec<String>, String> {
        let spec = CommandSpec::new(
            "docker",
            vec![
                kind.into(),
                "ls".into(),
                list.into(),
                "--filter".into(),
                format!("label=com.docker.compose.project={}", self.project),
            ],
            None,
            DIAGNOSTIC_TIMEOUT,
        );
        match self.runner.run(&spec) {
            Ok(output) if output.success && !output.truncated => Ok(output
                .stdout
                .split_whitespace()
                .map(str::to_owned)
                .collect()),
            _ => Err(format!("could not verify this run's {kind} resources")),
        }
    }

    /// Nothing owned by this run is left.
    pub fn all_removed(&self) -> Result<(), String> {
        let left: Vec<String> = self
            .remaining()?
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(kind, count)| format!("{count} {kind}(s)"))
            .collect();
        if left.is_empty() {
            Ok(())
        } else {
            Err(format!("this run left {} behind", left.join(", ")))
        }
    }
}

impl Drop for OwnedResources<'_> {
    fn drop(&mut self) {
        // Scoped to this run's own Compose project label, which is unique per
        // run, so this can never reach anything else.
        for (kind, list) in [("container", "-aq"), ("network", "-q"), ("volume", "-q")] {
            // Drop cannot report failure. It must not remove resources based
            // on incomplete inspection; all_removed reports that uncertainty.
            for id in self.ids(kind, list).unwrap_or_default() {
                let mut args = vec![kind.to_owned(), "rm".to_owned()];
                if kind == "container" {
                    args.push("-f".to_owned());
                }
                args.push(id);
                let _ =
                    self.runner
                        .run(&CommandSpec::new("docker", args, None, DIAGNOSTIC_TIMEOUT));
            }
        }
    }
}

/// A private configuration root and a project name nothing else can collide
/// with, so a run never touches a real installation of the same app.
pub struct Isolation {
    pub run_id: String,
    pub project: String,
    pub root: PathBuf,
}

impl Isolation {
    pub fn new(app: &str, scratch: &Path) -> AppResult<Self> {
        let run_id = format!(
            "{app}-qualify-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_nanos())
                .unwrap_or_default()
        );
        let root = scratch.join(&run_id);
        std::fs::create_dir_all(&root).map_err(AppError::from)?;
        Ok(Self {
            project: format!("local-store-{run_id}"),
            run_id,
            root,
        })
    }

    /// Point the process at this run's own configuration root.
    ///
    /// This is process-global, which is why qualification runs one app at a
    /// time. A batch that ran two apps at once would have them share a
    /// registry and each see the other as already installed.
    pub fn take_over_config_root(&self) {
        std::env::set_var("APPDATA", &self.root);
        std::env::set_var("XDG_CONFIG_HOME", &self.root);
    }

    pub fn registry_is_empty(&self) -> Result<(), String> {
        match crate::storage::load_registry_v2_at(&self.root) {
            Ok(registry) if registry.apps.is_empty() => Ok(()),
            Ok(registry) => Err(format!(
                "{} app(s) remained in the registry",
                registry.apps.len()
            )),
            Err(error) => Err(format!("the registry could not be read: {error}")),
        }
    }

    pub fn project_dir(&self) -> PathBuf {
        self.root
            .join(crate::brand::CONFIG_SLUG)
            .join("apps")
            .join(&self.run_id)
    }
}

/// Health, expressed as a step result rather than an error type, so a timeout
/// reads the same as any other failure in the report.
pub fn answered(probe: &dyn HealthProbe, address: &str, timeout: Duration) -> Result<(), String> {
    crate::runtime::wait_for_health_with(
        probe,
        address,
        timeout,
        &crate::runtime::CancelToken::new(),
    )
    .map_err(|_| format!("the app did not answer at {address} within {timeout:?}"))
}

/// Qualify one offered app end to end, against real Docker.
///
/// The sequence is the product promise, in order: it installs with one action,
/// answers, is usable, survives a restart, survives a reinstall that keeps the
/// person's data, and leaves nothing behind when it is removed. Each step is
/// recorded; the first failure stops the rest, because a later check run on a
/// broken app tells you nothing.
///
/// Every resource this creates is scoped to a project name unique to the run,
/// and removed when the run ends however it ends.
pub fn qualify(
    app: &str,
    answers: &BTreeMap<String, String>,
    first_use: &dyn FirstUse,
    scratch: &Path,
) -> AppResult<Evidence> {
    let offering = crate::offerings::offering(app)
        .ok_or_else(|| AppError::invalid(format!("{app} is not offered, so it cannot be run")))?;
    let reviewed = crate::templates::reviewed_template(app);
    let template = offering.plan_template(None)?;
    let (source_adapter, source_locator, source_observed_on, images_observed_on) = match &offering {
        crate::offerings::Offering::Recipe(recipe) => (
            "recipe".to_owned(),
            recipe.source_url.clone(),
            recipe.verified_at.clone(),
            recipe.requirements.image_audit.checked_at.clone(),
        ),
        crate::offerings::Offering::Template(template) => (
            template.origin.importer.clone(),
            format!("{}#{}", template.origin.repository, template.origin.path),
            template.verified_at.clone(),
            template
                .requirements
                .images
                .iter()
                .map(|audit| audit.checked_at.as_str())
                .min()
                .unwrap_or_default()
                .to_owned(),
        ),
    };
    let about = Subject {
        app: app.to_owned(),
        kind: if offering.is_recipe() {
            "reviewed recipe"
        } else {
            "reviewed mapping"
        },
        promotion: reviewed
            .as_ref()
            .map(|reviewed| reviewed.promotion.state.clone())
            .unwrap_or_else(|| "recipe".to_owned()),
        source_revision: reviewed
            .as_ref()
            .map(|reviewed| reviewed.origin.revision.clone())
            .unwrap_or_default(),
        source_adapter,
        source_locator,
        source_observed_on,
        images_observed_on,
    };
    qualify_template(&about, template, answers, first_use, scratch)
}

/// What a run is about, for the evidence it writes.
///
/// Kept separate from the template so the same run flow can qualify something
/// that is not offered yet. Gathering evidence about a candidate is how it
/// might one day be offered; it is not itself an offer, and nothing here
/// changes what `offerings` will resolve.
#[derive(Debug, Clone)]
pub struct Subject {
    pub app: String,
    pub kind: &'static str,
    pub promotion: String,
    pub source_revision: String,
    pub source_adapter: String,
    pub source_locator: String,
    pub source_observed_on: String,
    pub images_observed_on: String,
}

/// Qualify a template that has already been built.
/// The machine-wide qualification slot, held until dropped.
struct QualificationSlot(std::fs::File);

impl Drop for QualificationSlot {
    fn drop(&mut self) {
        let _ = fs4::FileExt::unlock(&self.0);
    }
}

/// One qualification at a time on this machine, across processes.
///
/// The bystander check notes every container that is not this run's and fails
/// if any of them disappear. A second harness running at the same time makes
/// and removes containers of its own, so each run's check blames the other:
/// Grafana failed "leaves other containers alone" because a concurrent batch
/// had just cleaned up after Joplin. Runs in one process are already
/// sequential — the configuration root is process-global — and this extends
/// the rule to every process. A second run waits its turn rather than failing.
fn take_qualification_slot(scratch: &Path) -> AppResult<QualificationSlot> {
    std::fs::create_dir_all(scratch).map_err(AppError::from)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(scratch.join("qualification.lock"))
        .map_err(AppError::from)?;
    fs4::FileExt::lock(&file).map_err(AppError::from)?;
    Ok(QualificationSlot(file))
}

/// How long each wait for an answer gets.
///
/// A review that found an app starts slowly was giving it that time only on
/// its very first start. Tandoor runs nginx in front of gunicorn and takes
/// longer than two minutes to answer again after a restart, so it failed
/// "survives a restart" while starting perfectly well. An allowance is about
/// the app, not about which start it is.
fn health_allowance(first_start: Option<Duration>) -> Duration {
    const DEFAULT: Duration = Duration::from_secs(120);
    match first_start {
        Some(reviewed) if reviewed > DEFAULT => reviewed,
        _ => DEFAULT,
    }
}

pub fn qualify_template(
    about: &Subject,
    template: PlanTemplate,
    answers: &BTreeMap<String, String>,
    first_use: &dyn FirstUse,
    scratch: &Path,
) -> AppResult<Evidence> {
    let app = about.app.as_str();
    // Taken first so it is released last, after every container this run
    // made is gone.
    let _slot = take_qualification_slot(scratch)?;
    let binding =
        crate::runtime::engine::EngineBinding::discover(&crate::runtime::SystemProcessRunner)?;
    let runner = crate::runtime::engine::EngineRunner {
        inner: &crate::runtime::SystemProcessRunner,
        binding: Some(binding.clone()),
    };
    let compose_version = runner
        .run(&CommandSpec::new(
            "docker",
            vec!["compose".into(), "version".into(), "--short".into()],
            None,
            DIAGNOSTIC_TIMEOUT,
        ))
        .ok()
        .filter(|output| output.success && !output.truncated)
        .map(|output| output.stdout.trim().to_owned())
        .unwrap_or_default();
    let probe = crate::runtime::HttpHealthProbe;
    let health = health_allowance(template.first_start);
    let mut template = template;
    let display_name = app.to_owned();

    let isolation = Isolation::new(app, scratch)?;
    isolation.take_over_config_root();
    template.plan.id = isolation.run_id.clone();
    // A port nothing else holds, so a run never collides with a real install.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(AppError::from)?;
    let port = listener.local_addr().map_err(AppError::from)?.port();
    drop(listener);
    template.plan.set_published_host(port);

    let images: Vec<String> = template
        .plan
        .services
        .iter()
        .map(|service| service.image.clone())
        .collect();

    let bystanders = Bystanders::note(&runner)?;
    let owned = OwnedResources::new(&runner, &isolation.project);
    let mut steps = Steps::new();

    for image in &images {
        let image = image.clone();
        steps.run(format!("pull {image}"), || {
            let output = runner
                .run(&CommandSpec::new(
                    "docker",
                    vec!["pull".into(), image.clone()],
                    None,
                    crate::runtime::PROVISION_TIMEOUT,
                ))
                .map_err(|error| error.message)?;
            output
                .success
                .then_some(())
                .ok_or_else(|| format!("{image} could not be pulled"))
        });
    }

    let first_start = std::time::Instant::now();
    let installed = steps.run("installs with one action", || {
        crate::runtime::install_template_on_engine(&template, &display_name, answers, &binding)
            .map_err(|error| error.message)
    });
    let Some(installed) = installed else {
        return Ok(finish(
            about,
            (&template, &binding, &compose_version),
            images,
            BTreeMap::new(),
            None,
            first_use,
            steps,
        ));
    };

    let answered_on_first_start = steps.run("answers on its address", || {
        answered(&probe, &installed.launch_url, health)
    });
    let mut measurements =
        answered_on_first_start.map(|()| {
            ResourceMeasurements::new(
                first_start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
            )
        });
    steps.run("all services are ready after install", || {
        service_health::wait(&runner, &template.plan, &isolation.project, health)
    });
    if let Some(sample) = steps.run("measures resources after install", || {
        resource_usage::snapshot(&runner, &isolation.project)
    }) {
        if let Some(measurements) = &mut measurements {
            measurements.observe(sample);
        }
    }
    steps.run(format!("is usable {}", Phase::FirstInstall.label()), || {
        first_use.exercise(Phase::FirstInstall, &installed.launch_url)
    });

    // What a person would lose. Captured before anything is removed so the
    // reinstall can be compared against it.
    let project_dir = isolation.project_dir();
    let secrets_before =
        std::fs::read_to_string(project_dir.join(crate::runtime::SECRETS_FILE)).ok();

    steps.run("survives a restart", || {
        crate::runtime::stop_with(&runner, &installed).map_err(|error| error.message)?;
        crate::runtime::start_with(&runner, &installed).map_err(|error| error.message)?;
        answered(&probe, &installed.launch_url, health)
    });
    steps.run(format!("is usable {}", Phase::AfterRestart.label()), || {
        first_use.exercise(Phase::AfterRestart, &installed.launch_url)
    });
    steps.run("all services are ready after restart", || {
        service_health::wait(&runner, &template.plan, &isolation.project, health)
    });
    if let Some(sample) = steps.run("measures resources after restart", || {
        resource_usage::snapshot(&runner, &isolation.project)
    }) {
        if let Some(measurements) = &mut measurements {
            measurements.observe(sample);
        }
    }

    let again = steps.run("reinstalls over data it kept", || {
        crate::runtime::uninstall_and_remove(&installed, false).map_err(|error| error.message)?;
        let again =
            crate::runtime::install_template_on_engine(&template, &display_name, answers, &binding)
                .map_err(|error| error.message)?;
        answered(&probe, &again.launch_url, health)?;
        Ok(again)
    });

    // The failure that looks exactly like data loss: a reinstall that mints
    // fresh credentials cannot open the data it was reinstalled onto.
    //
    // An app that generates nothing passes this trivially, so it is named for
    // what was actually checked. Evidence that reads "keeps the credentials it
    // generated" for an app with no credentials is evidence nobody can trust.
    let credential_step = if secrets_before.is_some() {
        "keeps the credentials it generated"
    } else {
        "generates no credentials, so there are none to lose"
    };
    steps.run(credential_step, || {
        let after = std::fs::read_to_string(project_dir.join(crate::runtime::SECRETS_FILE)).ok();
        match (&secrets_before, &after) {
            (None, None) => Ok(()),
            (Some(before), Some(after)) if before == after => Ok(()),
            (Some(_), Some(_)) => {
                Err("the reinstall replaced the credentials it should have reused".to_owned())
            }
            (Some(_), None) => Err("the reinstall lost the credential file".to_owned()),
            (None, Some(_)) => Err("credentials appeared that the install never had".to_owned()),
        }
    });

    if let Some(again) = &again {
        steps.run("all services are ready after reinstall", || {
            service_health::wait(&runner, &template.plan, &isolation.project, health)
        });
        if let Some(sample) = steps.run("measures resources after reinstall", || {
            resource_usage::snapshot(&runner, &isolation.project)
        }) {
            if let Some(measurements) = &mut measurements {
                measurements.observe(sample);
            }
        }
        steps.run(
            format!("is usable {}", Phase::AfterReinstall.label()),
            || first_use.exercise(Phase::AfterReinstall, &again.launch_url),
        );
    }

    let image_ids = resolved_images(&runner, &isolation.project);

    steps.run("removes everything it created", || {
        let target = again.as_ref().unwrap_or(&installed);
        crate::runtime::uninstall_and_remove(target, true).map_err(|error| error.message)?;
        owned.all_removed()?;
        if project_dir.exists() {
            return Err("the app's data directory survived a deletion".to_owned());
        }
        isolation.registry_is_empty()
    });

    steps.run("leaves other containers alone", || {
        bystanders.survived(&runner)
    });

    Ok(finish(
        about,
        (&template, &binding, &compose_version),
        images,
        image_ids,
        measurements,
        first_use,
        steps,
    ))
}

/// Container image ids for what this run actually ran, so evidence names the
/// bytes rather than a tag that may since have moved.
fn resolved_images(runner: &dyn ProcessRunner, project: &str) -> BTreeMap<String, String> {
    let spec = CommandSpec::new(
        "docker",
        vec![
            "container".into(),
            "ls".into(),
            "--all".into(),
            "--filter".into(),
            format!("label=com.docker.compose.project={project}"),
            "--format".into(),
            "{{.Image}} {{.ID}}".into(),
        ],
        None,
        DIAGNOSTIC_TIMEOUT,
    );
    let Ok(output) = runner.run(&spec) else {
        return BTreeMap::new();
    };
    let mut found = BTreeMap::new();
    for line in output.stdout.lines() {
        if let Some((image, container)) = line.trim().split_once(' ') {
            let inspect = CommandSpec::new(
                "docker",
                vec![
                    "inspect".into(),
                    container.into(),
                    "--format".into(),
                    "{{.Image}}".into(),
                ],
                None,
                DIAGNOSTIC_TIMEOUT,
            );
            if let Ok(id) = runner.run(&inspect) {
                if id.success {
                    found.insert(image.to_owned(), id.stdout.trim().to_owned());
                }
            }
        }
    }
    found
}

fn finish(
    about: &Subject,
    runtime: (&PlanTemplate, &crate::runtime::engine::EngineBinding, &str),
    images: Vec<String>,
    image_ids: BTreeMap<String, String>,
    measurements: Option<ResourceMeasurements>,
    first_use: &dyn FirstUse,
    steps: Steps,
) -> Evidence {
    let (template, binding, compose_version) = runtime;
    let results = steps.into_results();
    let digest = |value: &[u8]| {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(value))
    };
    let plan = template.plan.to_compose().unwrap_or_default();
    let identity = EvidenceIdentity {
        source_kind: about.kind.to_owned(),
        source_adapter: about.source_adapter.clone(),
        source_locator: about.source_locator.clone(),
        source_revision: about.source_revision.clone(),
        source_observed_on: about.source_observed_on.clone(),
        images_observed_on: about.images_observed_on.clone(),
        plan_sha256: digest(plan.as_bytes()),
        requested_images: images.clone(),
        resolved_image_ids: image_ids.clone(),
        host_os: std::env::consts::OS.to_owned(),
        host_arch: std::env::consts::ARCH.to_owned(),
        engine: binding.clone(),
        compose_version: compose_version.to_owned(),
        probe_sha256: digest(first_use.describes().as_bytes()),
        level: "lifecycle_and_first_use".into(),
    };
    Evidence {
        schema_version: 2,
        recorded_at_unix: unix_now(),
        app: about.app.clone(),
        passed: results.iter().all(|step| step.passed),
        scope: format!(
            "{}, transaction, managed storage and first use, on one host and one architecture",
            about.kind
        ),
        promotion: about.promotion.clone(),
        source_revision: about.source_revision.clone(),
        images,
        image_ids,
        first_use: first_use.describes().to_owned(),
        measurements,
        steps: results,
        identity: Some(identity),
    }
}

/// What a resumed batch should do about apps it already has an answer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resume {
    /// Anything with a recorded result is done. This is what "without
    /// repeating completed work" means: a failure is an answer too.
    SkipRecorded,
    /// Try the failures again, keeping the passes. For when the failures were
    /// about the machine rather than the app.
    RetryFailures,
    /// Run everything asked for, recorded or not. For when the question has
    /// changed rather than the answer — proving a different packaging of an
    /// app already qualified, where the existing pass is about a definition
    /// that is no longer the one under consideration.
    RunAnyway,
}

/// A directory of qualification results that a run can be resumed from.
///
/// Qualifying a shortlist takes hours of real Docker time, and the run will be
/// interrupted — a laptop sleeps, Docker restarts, somebody presses Ctrl-C.
/// Starting again from the top is the difference between a batch that gets
/// finished and one that does not.
pub struct Batch {
    dir: PathBuf,
}

impl Batch {
    pub fn open(dir: impl Into<PathBuf>) -> AppResult<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(AppError::from)?;
        Ok(Self { dir })
    }

    fn path(&self, app: &str) -> PathBuf {
        self.dir.join(format!("{app}.json"))
    }

    /// The result recorded for this app, if there is a readable one.
    ///
    /// An unreadable file is treated as no result rather than as an error: a
    /// run killed mid-write should be redone, not block the batch forever.
    pub fn recorded(&self, app: &str) -> Option<Evidence> {
        let text = std::fs::read_to_string(self.path(app)).ok()?;
        let evidence: Evidence = serde_json::from_str(&text).ok()?;
        // Keep old JSON readable for history, but never resume a stronger
        // harness from evidence that did not run its checks. Unknown future
        // versions also require an explicit migration instead of a guess.
        (evidence.schema_version == 2 && evidence.app == app && evidence.identity.is_some())
            .then_some(evidence)
    }

    /// Return evidence only when every proof-defining input still matches.
    pub fn recorded_current(&self, app: &str, identity: &EvidenceIdentity) -> Option<Evidence> {
        self.recorded(app)
            .filter(|evidence| evidence.is_current(identity))
    }

    /// Write a result, atomically, so an interruption cannot leave a half file
    /// that a later resume would read as an answer.
    pub fn record(&self, evidence: &Evidence) -> AppResult<()> {
        let target = self.path(&evidence.app);
        let staging = target.with_extension("json.partial");
        std::fs::write(
            &staging,
            evidence.to_json()
                + "
",
        )
        .map_err(AppError::from)?;
        std::fs::rename(&staging, &target).map_err(AppError::from)?;
        Ok(())
    }

    /// The apps still to run, in the order given.
    pub fn remaining<'a>(&self, apps: &'a [String], resume: Resume) -> Vec<&'a String> {
        apps.iter()
            .filter(|app| match (self.recorded(app), resume) {
                (None, _) => true,
                (Some(_), Resume::SkipRecorded) => false,
                (Some(evidence), Resume::RetryFailures) => !evidence.passed,
                (Some(_), Resume::RunAnyway) => true,
            })
            .collect()
    }

    /// Schedule against the identity the caller intends to prove now. A result
    /// for the same app but different inputs is stale and must run again.
    pub fn remaining_current<'a>(
        &self,
        subjects: &'a [(String, EvidenceIdentity)],
        resume: Resume,
    ) -> Vec<&'a String> {
        subjects
            .iter()
            .filter(
                |(app, identity)| match (self.recorded_current(app, identity), resume) {
                    (None, _) => true,
                    (Some(_), Resume::SkipRecorded) => false,
                    (Some(evidence), Resume::RetryFailures) => !evidence.passed,
                    (Some(_), Resume::RunAnyway) => true,
                },
            )
            .map(|(app, _)| app)
            .collect()
    }

    /// Every result recorded so far, for a summary of where a batch got to.
    pub fn results(&self) -> Vec<Evidence> {
        let mut found: Vec<Evidence> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .filter_map(|text| serde_json::from_str::<Evidence>(&text).ok())
            .collect();
        found.sort_by(|a, b| a.app.cmp(&b.app));
        found
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_reviewed_allowance_covers_every_wait_not_only_the_first_start() {
        use super::health_allowance;
        use std::time::Duration;
        assert_eq!(health_allowance(None), Duration::from_secs(120));
        // Never shorter than what every app gets.
        assert_eq!(
            health_allowance(Some(Duration::from_secs(30))),
            Duration::from_secs(120)
        );
        assert_eq!(
            health_allowance(Some(Duration::from_secs(600))),
            Duration::from_secs(600)
        );
    }

    use super::*;

    #[test]
    fn a_run_stops_at_its_first_failure_rather_than_reporting_later_nonsense() {
        let mut steps = Steps::new();
        assert_eq!(steps.run("install", || Ok::<_, String>(1)), Some(1));
        assert_eq!(
            steps.run("health", || Err::<i32, _>("no answer".to_owned())),
            None
        );
        // Everything after the failure is skipped, not recorded as passing.
        assert_eq!(steps.run("data survived", || Ok::<_, String>(2)), None);
        let results = steps.into_results();
        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(!results[1].passed);
        assert_eq!(results[1].step, "health");
    }

    /// A failure is exactly when a container prints its configuration, so what
    /// gets written down has to go through redaction.
    #[test]
    fn a_failure_detail_is_redacted_before_it_is_recorded() {
        let mut steps = Steps::new();
        steps.run("start", || {
            Err::<(), _>("POSTGRES_PASSWORD=hunter2-should-not-appear".to_owned())
        });
        let recorded = &steps.into_results()[0];
        let detail = recorded.detail.as_deref().unwrap_or_default();
        assert!(
            !detail.contains("hunter2-should-not-appear"),
            "a secret reached the evidence: {detail}"
        );
    }

    #[test]
    fn a_passing_step_records_no_detail_at_all() {
        let mut steps = Steps::new();
        steps.run("install", || Ok::<_, String>(()));
        let recorded = &steps.into_results()[0];
        assert!(recorded.detail.is_none());
        // And it does not appear in the serialized form either.
        let json = serde_json::to_string(recorded).unwrap();
        assert!(!json.contains("detail"), "{json}");
    }

    #[test]
    fn evidence_is_deterministic_and_names_its_failing_step() {
        let evidence = Evidence {
            schema_version: 2,
            recorded_at_unix: unix_now(),
            app: "example".into(),
            passed: false,
            scope: "one host".into(),
            promotion: "withheld".into(),
            source_revision: "0".repeat(40),
            images: vec!["example/app:1.0".into()],
            image_ids: [("example/app:1.0".to_owned(), "sha256:abc".to_owned())]
                .into_iter()
                .collect(),
            first_use: "it opens".into(),
            measurements: None,
            steps: vec![
                StepResult {
                    step: "install".into(),
                    passed: true,
                    detail: None,
                },
                StepResult {
                    step: "health".into(),
                    passed: false,
                    detail: Some("no answer".into()),
                },
            ],
            identity: None,
        };
        assert_eq!(
            evidence.failure().map(|step| step.step.as_str()),
            Some("health")
        );
        // The same run has to produce the same bytes, or a diff cannot tell a
        // real change from a re-run.
        assert_eq!(evidence.to_json(), evidence.to_json());
        let parsed: Evidence = serde_json::from_str(&evidence.to_json()).unwrap();
        assert_eq!(parsed, evidence);
    }

    #[test]
    fn a_first_use_check_runs_at_every_phase_with_the_address_it_was_given() {
        struct Recording(std::sync::Mutex<Vec<(Phase, String)>>);
        impl FirstUse for Recording {
            fn describes(&self) -> &str {
                "records"
            }
            fn exercise(&self, phase: Phase, address: &str) -> Result<(), String> {
                self.0.lock().unwrap().push((phase, address.to_owned()));
                Ok(())
            }
        }
        let probe = Recording(std::sync::Mutex::new(Vec::new()));
        for phase in [
            Phase::FirstInstall,
            Phase::AfterRestart,
            Phase::AfterReinstall,
        ] {
            probe.exercise(phase, "http://localhost:1234").unwrap();
        }
        let seen = probe.0.lock().unwrap();
        assert_eq!(seen.len(), 3);
        assert!(seen
            .iter()
            .all(|(_, address)| address == "http://localhost:1234"));
        assert_eq!(seen[2].0, Phase::AfterReinstall);
        assert_eq!(Phase::AfterReinstall.label(), "after a keep-data reinstall");
    }

    /// Answers a scripted sequence of Docker calls and records every one, so a
    /// test can prove what the harness did and did not ask for.
    struct FakeDocker {
        replies: std::sync::Mutex<Vec<String>>,
        calls: std::sync::Mutex<Vec<Vec<String>>>,
    }

    impl FakeDocker {
        fn new(replies: Vec<&str>) -> Self {
            Self {
                replies: std::sync::Mutex::new(replies.into_iter().map(str::to_owned).collect()),
                calls: std::sync::Mutex::new(Vec::new()),
            }
        }
        fn asked(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ProcessRunner for FakeDocker {
        fn run_cancellable(
            &self,
            spec: &CommandSpec,
            _: &crate::runtime::CancelToken,
        ) -> Result<crate::runtime::ProcessOutput, crate::runtime::ProcessError> {
            self.run(spec)
        }
        fn run(
            &self,
            spec: &CommandSpec,
        ) -> Result<crate::runtime::ProcessOutput, crate::runtime::ProcessError> {
            self.calls.lock().unwrap().push(spec.args.clone());
            // Only a listing answers with ids. Letting a removal consume a
            // scripted reply made this double lie about what the harness saw.
            let listing = spec.args.get(1).map(String::as_str) == Some("ls");
            let mut replies = self.replies.lock().unwrap();
            let stdout = if !listing || replies.is_empty() {
                String::new()
            } else {
                replies.remove(0)
            };
            Ok(crate::runtime::ProcessOutput {
                success: true,
                stdout,
                stderr: String::new(),
                truncated: false,
            })
        }
    }

    /// The one unacceptable outcome of a qualification run on somebody's real
    /// machine: removing a container that was not ours.
    #[test]
    fn a_container_belonging_to_something_else_going_missing_is_a_failure() {
        let before = FakeDocker::new(vec![
            "aaa
bbb
ccc",
        ]);
        let bystanders = Bystanders::note(&before).unwrap();

        let intact = FakeDocker::new(vec![
            "aaa
bbb
ccc
ddd",
        ]);
        assert!(
            bystanders.survived(&intact).is_ok(),
            "new containers of our own must not read as harm"
        );

        let harmed = FakeDocker::new(vec![
            "aaa
ccc",
        ]);
        let error = bystanders
            .survived(&harmed)
            .expect_err("a removed bystander must be reported");
        assert!(error.contains("1 container"), "{error}");
    }

    #[test]
    fn resources_this_run_owns_are_reported_when_they_are_left_behind() {
        // Containers gone, one network left.
        let runner = FakeDocker::new(vec!["", "net1", ""]);
        let owned = OwnedResources::new(&runner, "local-store-example-qualify");
        let error = owned
            .all_removed()
            .expect_err("a leftover network must be reported");
        assert!(error.contains("network"), "{error}");
    }

    #[test]
    fn unavailable_or_partial_inventory_never_proves_cleanup() {
        struct Unavailable(u8);
        impl ProcessRunner for Unavailable {
            fn run_cancellable(
                &self,
                _: &CommandSpec,
                _: &crate::runtime::CancelToken,
            ) -> Result<crate::runtime::ProcessOutput, crate::runtime::ProcessError> {
                if self.0 == 0 {
                    return Err(crate::runtime::ProcessError::new(
                        crate::runtime::ProcessErrorCode::TimedOut,
                        "inspection timed out",
                    ));
                }
                Ok(crate::runtime::ProcessOutput {
                    success: self.0 != 1,
                    stdout: String::new(),
                    stderr: String::new(),
                    truncated: self.0 == 2,
                })
            }
        }
        for failure in 0..3 {
            let runner = Unavailable(failure);
            let owned = OwnedResources::new(&runner, "local-store-example-qualify");
            assert!(owned
                .all_removed()
                .unwrap_err()
                .contains("could not verify"));
            assert!(Bystanders::note(&runner).is_err());
        }
    }

    /// The case that matters: a run that fails half way still has to clean up
    /// after itself, because that is the run most likely to leave something
    /// behind on a machine that is not a test machine.
    #[test]
    fn a_run_that_fails_part_way_still_removes_what_it_created() {
        let runner = FakeDocker::new(vec!["c1", "n1", "v1"]);
        {
            let _owned = OwnedResources::new(&runner, "local-store-example-qualify");
            let mut steps = Steps::new();
            steps.run("start", || Err::<(), _>("it never started".to_owned()));
            assert!(steps.failed());
            // The guard goes out of scope here, with the run already failed.
        }
        let asked = runner.asked();
        let removals = asked
            .iter()
            .filter(|args| args.get(1).map(String::as_str) == Some("rm"))
            .count();
        assert_eq!(removals, 3, "{asked:?}");
        // Every removal is scoped to this run's own project label.
        for args in &asked {
            if let Some(filter) = args.iter().position(|arg| arg == "--filter") {
                assert!(
                    args[filter + 1].contains("local-store-example-qualify"),
                    "{args:?}"
                );
            }
        }
    }

    /// An app that never answers has to end the run, not hang it.
    #[test]
    fn an_app_that_never_answers_times_out_as_an_ordinary_step_failure() {
        struct Never;
        impl HealthProbe for Never {
            fn ready(&self, _: &str) -> bool {
                false
            }
        }
        let started = std::time::Instant::now();
        let error = answered(&Never, "http://localhost:1", Duration::from_millis(200))
            .expect_err("a silent app must time out");
        assert!(error.contains("did not answer"), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the timeout was not honoured"
        );
    }

    fn evidence_for(app: &str, passed: bool) -> Evidence {
        Evidence {
            schema_version: 2,
            recorded_at_unix: unix_now(),
            app: app.into(),
            passed,
            scope: "one host".into(),
            promotion: "withheld".into(),
            source_revision: "0".repeat(40),
            images: vec!["example/app:1.0".into()],
            image_ids: BTreeMap::new(),
            first_use: "it opens".into(),
            measurements: Some(ResourceMeasurements {
                first_start_millis: 1_000,
                samples: 3,
                idle_memory_bytes: [("example-1".into(), 1_024)].into_iter().collect(),
                peak_memory_bytes: [("example-1".into(), 2_048)].into_iter().collect(),
                peak_total_memory_bytes: 2_048,
            }),
            steps: vec![StepResult {
                step: "install".into(),
                passed,
                detail: (!passed).then(|| "it did not install".to_owned()),
            }],
            identity: Some(sample_identity()),
        }
    }

    fn sample_identity() -> EvidenceIdentity {
        let today = date_for_unix(unix_now());
        EvidenceIdentity {
            source_kind: "reviewed mapping".into(),
            source_adapter: "runtipi".into(),
            source_locator: "https://example.test/store#apps/example/docker-compose.json".into(),
            source_revision: "0".repeat(40),
            source_observed_on: today.clone(),
            images_observed_on: today,
            plan_sha256: "1".repeat(64),
            requested_images: vec!["example/app:1.0".into()],
            resolved_image_ids: BTreeMap::new(),
            host_os: "windows".into(),
            host_arch: "x86_64".into(),
            engine: crate::runtime::engine::EngineBinding {
                schema_version: 1,
                program: "docker".into(),
                endpoint: "npipe:////./pipe/docker_engine".into(),
            },
            compose_version: "5.5.1".into(),
            probe_sha256: "2".repeat(64),
            level: "lifecycle_and_first_use".into(),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "local-store-batch-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn old_or_future_evidence_cannot_skip_the_current_harness() {
        let dir = scratch("evidence-version");
        let batch = Batch::open(&dir).unwrap();
        let mut evidence = evidence_for("example", true);
        for version in [0, 1, 3] {
            evidence.schema_version = version;
            batch.record(&evidence).unwrap();
            assert!(batch.recorded("example").is_none());
        }
        let mut legacy = serde_json::to_value(&evidence).unwrap();
        legacy.as_object_mut().unwrap().remove("schema_version");
        std::fs::write(batch.path("example"), legacy.to_string()).unwrap();
        assert_eq!(
            serde_json::from_value::<Evidence>(legacy)
                .unwrap()
                .schema_version,
            0
        );
        assert!(batch.recorded("example").is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Qualifying a shortlist is hours of real Docker time and the run will be
    /// interrupted. Starting from the top each time is the difference between
    /// a batch that gets finished and one that does not.
    #[test]
    fn a_resumed_batch_does_not_repeat_work_it_already_has_an_answer_for() {
        let dir = scratch("resume");
        let batch = Batch::open(&dir).unwrap();
        let apps: Vec<String> = ["one", "two", "three"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(batch.remaining(&apps, Resume::SkipRecorded).len(), 3);

        batch.record(&evidence_for("one", true)).unwrap();
        batch.record(&evidence_for("two", false)).unwrap();

        // A failure is an answer: skipping it is what "completed work" means.
        let left = batch.remaining(&apps, Resume::SkipRecorded);
        assert_eq!(left, vec![&"three".to_owned()]);

        // Unless the failures are worth another try, and then the passes stay
        // done.
        let retry = batch.remaining(&apps, Resume::RetryFailures);
        assert_eq!(retry, vec![&"two".to_owned(), &"three".to_owned()]);

        // And a new question about an app reruns even its pass: the pass was
        // about a different definition.
        let anyway = batch.remaining(&apps, Resume::RunAnyway);
        assert_eq!(anyway.len(), 3);

        let all = batch.results();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].app, "one");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn changed_proof_identity_cannot_reuse_a_previous_pass() {
        let dir = scratch("identity-change");
        let batch = Batch::open(&dir).unwrap();
        let evidence = evidence_for("example", true);
        batch.record(&evidence).unwrap();
        let current = evidence.identity.clone().unwrap();
        assert!(batch.recorded_current("example", &current).is_some());
        let mut without_measurements = evidence.clone();
        without_measurements.measurements = None;
        assert!(!without_measurements.is_current(&current));

        let mut changed = current.clone();
        changed.plan_sha256 = "3".repeat(64);
        assert!(batch.recorded_current("example", &changed).is_none());
        let mut changed = current.clone();
        changed.compose_version = "6.0.0".into();
        assert!(batch.recorded_current("example", &changed).is_none());
        let mut changed = current.clone();
        changed.host_arch = "aarch64".into();
        assert!(batch.recorded_current("example", &changed).is_none());
        let mut changed = current;
        changed.probe_sha256 = "4".repeat(64);
        assert!(batch.recorded_current("example", &changed).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn batch_scheduling_reruns_stale_identity_but_keeps_current_passes() {
        let dir = scratch("identity-schedule");
        let batch = Batch::open(&dir).unwrap();
        batch.record(&evidence_for("one", true)).unwrap();
        batch.record(&evidence_for("two", false)).unwrap();
        let current = sample_identity();
        let mut stale = current.clone();
        stale.engine = crate::runtime::engine::EngineBinding::managed_wsl();
        let subjects = vec![
            ("one".into(), current.clone()),
            ("two".into(), current),
            ("three".into(), stale.clone()),
        ];
        assert_eq!(
            batch.remaining_current(&subjects, Resume::SkipRecorded),
            vec![&"three".to_owned()]
        );
        assert_eq!(
            batch.remaining_current(&subjects, Resume::RetryFailures),
            vec![&"two".to_owned(), &"three".to_owned()]
        );
        let changed = vec![("one".into(), stale)];
        assert_eq!(
            batch.remaining_current(&changed, Resume::SkipRecorded),
            vec![&"one".to_owned()]
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn expired_missing_or_future_dated_proof_is_never_current() {
        let now = 2_000_000_000;
        let mut identity = sample_identity();
        identity.source_observed_on = date_for_unix(now);
        identity.images_observed_on = date_for_unix(now);
        let mut evidence = evidence_for("example", true);
        evidence.identity = Some(identity.clone());
        evidence.recorded_at_unix = now - Evidence::MAX_REUSE_AGE_SECS;
        assert!(evidence.is_current_at(&identity, now));
        evidence.recorded_at_unix -= 1;
        assert!(!evidence.is_current_at(&identity, now));
        evidence.recorded_at_unix = 0;
        assert!(!evidence.is_current_at(&identity, now));
        evidence.recorded_at_unix = now + Evidence::MAX_FUTURE_SKEW_SECS + 1;
        assert!(!evidence.is_current_at(&identity, now));
    }

    #[test]
    fn source_and_image_observations_expire_independently() {
        let now = 2_000_000_000;
        let today = date_for_unix(now);
        let old_source = date_for_unix(now - (Evidence::MAX_SOURCE_AGE_DAYS as u64 + 1) * 86_400);
        let old_image = date_for_unix(now - (Evidence::MAX_IMAGE_AGE_DAYS as u64 + 1) * 86_400);
        let mut evidence = evidence_for("example", true);
        evidence.recorded_at_unix = now;
        let mut identity = evidence.identity.clone().unwrap();
        identity.source_observed_on = today.clone();
        identity.images_observed_on = today.clone();
        evidence.identity = Some(identity.clone());
        assert!(evidence.is_current_at(&identity, now));

        identity.source_observed_on = old_source;
        evidence.identity = Some(identity.clone());
        assert!(!evidence.is_current_at(&identity, now));
        identity.source_observed_on = today;
        identity.images_observed_on = old_image;
        evidence.identity = Some(identity.clone());
        assert!(!evidence.is_current_at(&identity, now));
    }

    #[test]
    fn a_failed_probe_is_recorded_by_what_it_threw() {
        let node = concat!(
            "file:///probe.mjs:43\n",
            "    throw new Error(`the address answered ${status}`);\n",
            "          ^\n",
            "\n",
            "Error: the address answered 500\n",
            "    at file:///probe.mjs:43:11\n",
            "\n",
            "Node.js v24.14.0\n",
        );
        assert_eq!(probe_failure(node), "Error: the address answered 500");
        // Something that threw nothing recognisable still reads forwards.
        assert_eq!(probe_failure("one\ntwo\nNode.js v24\n"), "one two");
    }

    /// Two harnesses at once each blame the other for removing containers,
    /// so the slot has to keep a second one out while the first holds it.
    #[test]
    fn a_second_qualification_waits_for_the_first() {
        let dir = std::env::temp_dir().join(format!("local-store-slot-{}", std::process::id()));
        let held = take_qualification_slot(&dir).expect("the first run takes the slot");
        let second = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.join("qualification.lock"))
            .unwrap();
        assert!(
            fs4::FileExt::try_lock(&second).is_err(),
            "a second run got the slot while the first held it"
        );
        drop(held);
        assert!(
            fs4::FileExt::try_lock(&second).is_ok(),
            "the slot was not released when the first run finished"
        );
        let _ = fs4::FileExt::unlock(&second);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A run killed mid-write must be redone rather than read as an answer,
    /// and must not stop the rest of the batch.
    #[test]
    fn a_half_written_result_counts_as_no_result() {
        let dir = scratch("partial");
        let batch = Batch::open(&dir).unwrap();
        std::fs::write(dir.join("one.json"), b"{\"app\": \"one\", \"pas").unwrap();
        assert!(batch.recorded("one").is_none());
        let apps = vec!["one".to_owned()];
        assert_eq!(batch.remaining(&apps, Resume::SkipRecorded).len(), 1);
        assert!(batch.results().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Writing is atomic, so an interruption leaves either the old answer or
    /// the new one, never half of either.
    #[test]
    fn recording_a_result_leaves_no_partial_file_behind() {
        let dir = scratch("atomic");
        let batch = Batch::open(&dir).unwrap();
        batch.record(&evidence_for("one", true)).unwrap();
        let stray: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.ends_with(".json"))
            .collect();
        assert!(stray.is_empty(), "{stray:?}");
        assert_eq!(batch.recorded("one").unwrap(), evidence_for("one", true));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_app_with_no_first_use_check_says_so_rather_than_claiming_more() {
        assert!(AnswersOnly.describes().contains("no first-use check"));
        assert!(AnswersOnly
            .exercise(Phase::FirstInstall, "http://localhost:1")
            .is_ok());
    }
}
