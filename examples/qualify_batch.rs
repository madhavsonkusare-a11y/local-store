//! Run ranked candidates through the qualification harness, in bulk.
//!
//! Everything up to now has said what *could* be installed. This says what
//! actually works, which is the only number the v1 goal depends on and the one
//! nobody has. It installs each candidate for real, uses it, restarts it,
//! reinstalls it over its own data, and removes it — then records what
//! happened.
//!
//! **This is evidence gathering, not offering.** A candidate qualified here is
//! not thereby installable by anybody: `offerings` still resolves only
//! reviewed recipes and approved templates, and nothing in this example
//! changes that. It exists so a promotion decision has something to read.
//!
//! Resumable, because a hundred apps is hours and something will interrupt it.
//! Run again and it picks up where it stopped.
//!
//!     LOCAL_STORE_RUN_DOCKER_TEST=1 cargo run --release --example qualify_batch -- --limit 10
//!
//! `--only app,app` runs named candidates and re-runs them even if a
//! result exists; `--source runtipi` picks which packaging of them to prove.
//! `--offered --only app` proves an app as it is offered, image pins and all;
//! `--probe scripts/app-probe.mjs` adds that app's own check.
use local_store::qualification::{qualify_template, Batch, Evidence, Resume, ScriptProbe, Subject};
use local_store::setup::{FieldKind, PlanTemplate};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// One candidate worth running, read from the ranking.
struct Candidate {
    id: String,
    source: String,
    revision: String,
    path: String,
}

fn ranked(limit: usize, only: &[String], source: Option<&str>) -> Vec<Candidate> {
    let ranking: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("catalog/candidate-ranking.json"))
            .expect("run scripts/rank-candidates.py first"),
    )
    .expect("the ranking is JSON");
    let queue: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("catalog/candidate-queue.json"))
            .expect("run scripts/build-candidate-queue.py first"),
    )
    .expect("the queue is JSON");

    // Provenance lives in the queue; reach and licence live in the ranking.
    let mut provenance: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    let mut required: BTreeMap<(String, String), u64> = BTreeMap::new();
    for entry in queue["candidates"].as_array().unwrap_or(&Vec::new()) {
        let key = (
            entry["id"].as_str().unwrap_or_default().to_owned(),
            entry["source"].as_str().unwrap_or_default().to_owned(),
        );
        provenance.insert(
            key.clone(),
            (
                entry["provenance"]["revision"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                entry["provenance"]["path"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            ),
        );
        required.insert(key, entry["required_inputs"].as_u64().unwrap_or(0));
    }

    let mut chosen = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in ranking["candidates"].as_array().unwrap_or(&Vec::new()) {
        if chosen.len() >= limit {
            break;
        }
        // A licence is recorded and reviewed in the manifest; it only orders
        // an unnamed run. Naming an app is a decision already made — Flowise,
        // whose repository reports NOASSERTION, was silently skipped here.
        if (only.is_empty() && entry["open_source"].as_bool() != Some(true))
            || !entry["importable"].as_bool().unwrap_or(false)
        {
            continue;
        }
        let key = (
            entry["id"].as_str().unwrap_or_default().to_owned(),
            entry["source"].as_str().unwrap_or_default().to_owned(),
        );
        // Apps that ask questions are no longer skipped: `auto_answers` fills
        // them, which matters because almost every required answer is a folder
        // and folders are most of what the popular apps need.
        let _ = &required;
        if !only.is_empty() && !only.contains(&key.0) {
            continue;
        }
        // Rank orders apps, not the definitions of one app, and the
        // higher-ranked source is not always the better-maintained package:
        // CapRover's grocy pinned an image from 2020 where Runtipi's was four
        // days old. `--source` is how a run says which packaging to prove.
        if source.is_some_and(|wanted| wanted != key.1) {
            continue;
        }
        // Otherwise one definition per app: two sources packaging the same
        // thing is one question, and the higher-ranked one is already first.
        if !seen.insert(key.0.clone()) {
            continue;
        }
        let Some((revision, path)) = provenance.get(&key) else {
            continue;
        };
        if revision.is_empty() || path.is_empty() {
            continue;
        }
        chosen.push(Candidate {
            id: key.0,
            source: key.1,
            revision: revision.clone(),
            path: path.clone(),
        });
    }
    chosen
}

/// The pinned definition and its companion config, extracted beside the
/// archives by `scripts/extract-definitions.py`.
fn definition(candidate: &Candidate) -> Option<(String, Option<String>)> {
    let base = root()
        .join(".cache/definitions")
        .join(&candidate.revision)
        .join(Path::new(&candidate.path).parent()?);
    // The extractor normalises CapRover's YAML into JSON, which is what its
    // importer reads.
    let mut file = PathBuf::from(Path::new(&candidate.path).file_name()?);
    if matches!(
        file.extension().and_then(|e| e.to_str()),
        Some("yml" | "yaml")
    ) {
        file.set_extension("json");
    }
    let definition = std::fs::read_to_string(base.join(file)).ok()?;
    let config = std::fs::read_to_string(base.join("config.json")).ok();
    Some((definition, config))
}

/// Whether this machine already has an image, so the batch can tell what it
/// pulled from what it found.
/// Free space on the drive this project lives on, in gigabytes.
fn free_gigabytes() -> Option<u64> {
    let free = fs4::available_space(root()).ok()?;
    Some(free / (1024 * 1024 * 1024))
}

fn image_present(image: &str) -> bool {
    std::process::Command::new("docker")
        .args(["image", "inspect", image])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Image families that are infrastructure rather than something a person
/// installs from a desktop store.
///
/// They rank high because everything pulls them, and they fail the standard
/// for the right reason — a bare web server serves a placeholder, a database
/// serves no page at all. Skipping them up front saves a run rather than
/// changing a verdict; the standard would refuse them anyway.
const NOT_AN_APP: &[&str] = &[
    "library/nginx",
    "library/httpd",
    "library/mongo",
    "library/postgres",
    "library/mariadb",
    "library/mysql",
    "library/redis",
    "library/memcached",
    "library/traefik",
    "library/haproxy",
    "library/rabbitmq",
    "library/influxdb",
    "library/elasticsearch",
];

/// Whether a definition is *only* infrastructure.
///
/// This asked whether *any* image was infrastructure, which skipped every app
/// that ships its own database: WordPress, Nextcloud, Joplin, Monica and
/// Guacamole were all passed over as "not an app" because of a Postgres or
/// MariaDB sidecar. An app is judged by what it is, not by what it stores its
/// data in — the same mistake the ranking once made with pull counts.
fn is_infrastructure(images: &[String]) -> bool {
    !images.is_empty()
        && images.iter().all(|image| {
            let repository = image
                .rsplit_once(':')
                .map_or(image.as_str(), |(name, _)| name);
            let repository = if repository.contains('/') {
                repository.to_owned()
            } else {
                format!("library/{repository}")
            };
            NOT_AN_APP.iter().any(|known| repository == *known)
        })
}

/// Answers a batch can supply for itself, so an app that asks a question is
/// not simply skipped.
///
/// Thirty-six of the open-source importable candidates need an answer, and
/// almost all of those answers are a folder — which is exactly what a person
/// would pick. Supplying them here is what makes those apps testable at all.
/// It proves the app installs, opens and keeps its data; it does not prove a
/// person chose well, and the evidence says the answers were supplied
/// automatically so nobody reads it as more than it is.
fn auto_answers(template: &PlanTemplate, scratch: &Path) -> BTreeMap<String, String> {
    let mut answers = BTreeMap::new();
    for field in &template.fields {
        if !field.required {
            continue;
        }
        let key = field.key.to_ascii_uppercase();
        let value = match &field.kind {
            FieldKind::Folder { .. } => {
                // A real, empty folder of its own, which is what a person
                // pointing at their music would be doing.
                let folder = scratch.join("shared").join(field.key.to_ascii_lowercase());
                if std::fs::create_dir_all(&folder).is_err() {
                    continue;
                }
                folder.to_string_lossy().into_owned()
            }
            FieldKind::Choice { options } => match options.first() {
                Some(first) => first.clone(),
                None => continue,
            },
            FieldKind::Boolean => "false".to_owned(),
            FieldKind::Number { min, .. } => min.to_string(),
            FieldKind::Text { min_len, max_len }
            | FieldKind::Pattern {
                min_len, max_len, ..
            } => {
                if let Some(default) = &field.default {
                    default.clone()
                } else if key.contains("EMAIL") {
                    "probe@example.invalid".to_owned()
                } else if key.contains("PASSWORD") || key.contains("SECRET") {
                    // Long enough for the strictest rule seen upstream, and
                    // thrown away with the isolated root it lives in.
                    "Qualify-probe-9271-not-a-real-secret".to_owned()
                } else if key.contains("USER") || key.contains("NAME") {
                    "local-store-probe".to_owned()
                } else {
                    let filler = "qualification-probe";
                    let clamped = filler.chars().take(*max_len).collect::<String>();
                    if clamped.chars().count() < *min_len {
                        format!("{clamped}{}", "x".repeat(min_len - clamped.chars().count()))
                    } else {
                        clamped
                    }
                }
            }
        };
        answers.insert(field.key.clone(), value);
    }
    answers
}

/// Whether a failure was about this machine rather than about the app.
///
/// A pull that ran out of time on a slow link, or a port that was busy, says
/// nothing about whether an app works. Counting those as app failures makes a
/// slow afternoon look like a bad catalogue, so they are retried once and
/// reported separately.
fn is_environmental(evidence: &Evidence) -> bool {
    let Some(failure) = evidence.failure() else {
        return false;
    };
    if failure.step.starts_with("pull ") {
        return true;
    }
    let detail = failure.detail.as_deref().unwrap_or("").to_ascii_lowercase();
    // A timed-out install now carries what its containers said, and that can
    // settle the question: an app that logged requests answered with 5xx, or
    // a container that exited, is the app failing — not the machine. Monica
    // answered every request with 500 for three minutes and was retried as
    // though Docker had been slow.
    let app_refused = detail.contains("exited (")
        || ["http/1.0\" 5", "http/1.1\" 5"]
            .iter()
            .any(|sign| detail.contains(sign));
    if app_refused {
        return false;
    }
    [
        "timed out",
        "timeout",
        "connection",
        "network",
        "already in use",
        "no space",
    ]
    .iter()
    .any(|sign| detail.contains(sign))
}

/// Why the manifest compiled into this binary is not the one on disk, if it
/// is not.
fn stale_manifest(app: &str) -> Option<String> {
    let embedded = local_store::templates::reviewed_template(app)?;
    let path = root().join("src/templates").join(format!("{app}.json"));
    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    let normal = |text: &str| text.replace("\r\n", "\n");
    if on_disk["definition"].as_str().map(normal) != Some(normal(&embedded.definition)) {
        return Some("its definition changed after this binary was built".into());
    }
    let embedded_config = embedded
        .config
        .as_ref()
        .map(|config| normal(&config.content));
    if on_disk["config"]["content"].as_str().map(normal) != embedded_config {
        return Some("its config changed after this binary was built".into());
    }
    let images: Vec<(String, Option<String>)> = embedded
        .requirements
        .images
        .iter()
        .map(|audit| (audit.image.clone(), audit.index_digest.clone()))
        .collect();
    let disk_images: Vec<(String, Option<String>)> = on_disk["requirements"]["images"]
        .as_array()?
        .iter()
        .map(|audit| {
            (
                audit["image"].as_str().unwrap_or_default().to_owned(),
                audit["index_digest"].as_str().map(str::to_owned),
            )
        })
        .collect();
    (images != disk_images).then(|| "its pinned images changed after this binary was built".into())
}

/// Prove apps exactly as they are offered, rather than as an importer maps them.
///
/// A candidate run proves the raw mapping. An offered app can differ from that
/// — an image pin moves it to a patched release — and the proof has to be
/// about what somebody would actually install. This runs the reviewed mapping
/// through the same harness and writes the result where the review names it.
/// It is also how an offered app is re-verified before a release.
fn run_offered(only: &[String], batch: &Batch, app_probe: Option<&str>) {
    if only.is_empty() {
        eprintln!("--offered needs --only app,app: it re-proves named apps, not a ranking");
        std::process::exit(2);
    }
    let scratch = root().join(".cache");
    for app in only {
        let Some(offering) = local_store::offerings::offering(app) else {
            println!("{app}: not offered, so there is nothing to prove as offered");
            continue;
        };
        // Manifests are compiled in. A binary built before a manifest was
        // regenerated proves the old one — Huginn and Langflow were each run
        // against definitions already replaced on disk, 21 seconds apart.
        if let Some(reason) = stale_manifest(app) {
            println!("{app}: {reason}; rebuild before proving it");
            continue;
        }
        let template = match offering.plan_template(None) {
            Ok(template) => template,
            Err(error) => {
                println!(
                    "{app}: its reviewed mapping does not resolve: {}",
                    error.message
                );
                continue;
            }
        };
        let answers = auto_answers(&template, &scratch);
        // An app's own probe checks what only that app does — Penpot's MCP
        // server, say — on top of the standard. It is named on the command
        // line rather than found by file name, because probes do not all
        // take the same arguments.
        let (script, describes) = match app_probe {
            Some(script) => (
                root().join(script),
                format!(
                    "it opens a page with a clear next step, the same page after a restart and reinstall, and {script} passed"
                ),
            ),
            None => (
                root().join("scripts/standard-probe.mjs"),
                "it opens a page with a clear next step, and the same page after a restart and reinstall"
                    .to_owned(),
            ),
        };
        let probe = ScriptProbe::new(script, describes).with_args(vec![scratch
            .join(format!("standard-{app}.json"))
            .to_string_lossy()
            .into_owned()]);
        println!("{app}: running as offered…");
        match local_store::qualification::qualify(app, &answers, &probe, &scratch) {
            Ok(evidence) => {
                if let Err(error) = batch.record(&evidence) {
                    println!("  could not record: {}", error.message);
                }
                let proof = root()
                    .join("docs/evidence")
                    .join(format!("{app}-qualification.json"));
                if let Err(error) = std::fs::write(&proof, evidence.to_json() + "\n") {
                    println!("  could not write {}: {error}", proof.display());
                }
                if evidence.passed {
                    println!("  passed");
                } else {
                    println!(
                        "  FAILED at {:?}: {:?}",
                        evidence.failure().map(|step| &step.step),
                        evidence.failure().and_then(|step| step.detail.as_deref())
                    );
                }
            }
            Err(error) => println!("  could not run: {}", error.message),
        }
    }
}

fn main() {
    if std::env::var("LOCAL_STORE_RUN_DOCKER_TEST").as_deref() != Ok("1") {
        eprintln!("This starts real containers. Set LOCAL_STORE_RUN_DOCKER_TEST=1 to run it.");
        std::process::exit(2);
    }
    let args: Vec<String> = std::env::args().collect();
    let limit = args
        .iter()
        .position(|arg| arg == "--limit")
        .and_then(|at| args.get(at + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(10usize);
    let keep_images = args.iter().any(|arg| arg == "--keep-images");
    // Naming apps is how a re-run proves one packaging against another, so it
    // implies retrying whatever was already recorded about them.
    let only: Vec<String> = args
        .iter()
        .position(|arg| arg == "--only")
        .and_then(|at| args.get(at + 1))
        .map(|value| value.split(',').map(|id| id.trim().to_owned()).collect())
        .unwrap_or_default();
    let source = args
        .iter()
        .position(|arg| arg == "--source")
        .and_then(|at| args.get(at + 1))
        .map(String::as_str);
    let resume = if !only.is_empty() {
        // Naming apps is asking a new question about them, so an old answer
        // does not settle it.
        Resume::RunAnyway
    } else if args.iter().any(|arg| arg == "--retry-failures") {
        Resume::RetryFailures
    } else {
        Resume::SkipRecorded
    };

    let results = root().join(".cache/qualification");
    let batch = Batch::open(&results).expect("a results directory");
    if args.iter().any(|arg| arg == "--offered") {
        let app_probe = args
            .iter()
            .position(|arg| arg == "--probe")
            .and_then(|at| args.get(at + 1))
            .map(String::as_str);
        run_offered(&only, &batch, app_probe);
        return;
    }
    let candidates = ranked(
        if only.is_empty() { limit } else { only.len() },
        &only,
        source,
    );
    if !only.is_empty() && candidates.len() < only.len() {
        let found: Vec<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
        for id in &only {
            if !found.contains(&id.as_str()) {
                println!(
                    "{id}: no importable candidate{}",
                    match source {
                        Some(name) => format!(" from {name}"),
                        None => String::new(),
                    }
                );
            }
        }
    }
    let ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();
    let todo = batch.remaining(&ids, resume);
    println!(
        "{} candidate(s) selected, {} already recorded, {} to run",
        ids.len(),
        ids.len() - todo.len(),
        todo.len()
    );

    let scratch = root().join(".cache");
    let (mut passed, mut failed, mut skipped) = (0, 0, 0);
    let mut environmental_failures = 0;
    for candidate in &candidates {
        if !todo.iter().any(|id| **id == candidate.id) {
            continue;
        }
        let Some((text, config)) = definition(candidate) else {
            println!("{}: no pinned definition on disk, skipping", candidate.id);
            skipped += 1;
            continue;
        };
        let outcome = match candidate.source.as_str() {
            "runtipi" => {
                local_store::importers::runtipi::import(&candidate.id, &text, config.as_deref())
            }
            "caprover" => local_store::importers::caprover::import(&candidate.id, &text),
            other => {
                println!("{}: no importer for {other}", candidate.id);
                skipped += 1;
                continue;
            }
        };
        let Some(mut template) = outcome.ok().and_then(|outcome| outcome.template) else {
            println!("{}: no longer importable, skipping", candidate.id);
            skipped += 1;
            continue;
        };
        // What Runtipi would have copied into the data folder, it gets here
        // too; a definition mounting one of these is broken without it.
        if candidate.source == "runtipi" {
            let folder = root()
                .join(".cache/definitions")
                .join(&candidate.revision)
                .join(Path::new(&candidate.path).parent().unwrap_or(Path::new("")));
            let (seeds, binary) = local_store::importers::runtipi::read_seeds(&folder);
            if !binary.is_empty() {
                println!("  {}: cannot carry binary seed(s) {binary:?}", candidate.id);
            }
            template.seeds = seeds;
        }
        // Qualifying a hundred apps means pulling a hundred images, which is
        // tens of gigabytes on somebody's real machine. Anything this run
        // pulls, it removes; anything that was already there is left exactly
        // as it was found.
        let images: Vec<String> = template
            .plan
            .services
            .iter()
            .map(|service| service.image.clone())
            .collect();
        if is_infrastructure(&images) {
            println!(
                "{}: infrastructure rather than an app, skipping",
                candidate.id
            );
            skipped += 1;
            continue;
        }
        let answers = auto_answers(&template, &scratch);
        let ours: Vec<String> = images
            .iter()
            .filter(|image| !image_present(image))
            .cloned()
            .collect();

        let about = Subject {
            app: candidate.id.clone(),
            kind: if answers.is_empty() {
                "candidate mapping"
            } else {
                "candidate mapping with automatically supplied answers"
            },
            // Nothing here is promoted. Saying so in the evidence keeps a
            // passing run from reading like an approval.
            promotion: "not reviewed".to_owned(),
            source_revision: candidate.revision.clone(),
            source_adapter: "candidate-import".into(),
            source_locator: candidate.id.clone(),
            // Candidate imports have no reviewed observation dates. Their
            // evidence remains useful history but cannot suppress a later run.
            source_observed_on: String::new(),
            images_observed_on: String::new(),
        };
        println!("{}: running…", candidate.id);
        // The App Store standard, applied to every app the same way: it has to
        // open something a person could act on, and it has to still be the same
        // page after a restart and a reinstall. Passing this makes an app
        // *tested*; only an app-specific probe makes one *verified*.
        let probe = ScriptProbe::new(
            root().join("scripts/standard-probe.mjs"),
            "it opens a page with a clear next step, and the same page after a restart and reinstall",
        )
        .with_args(vec![scratch
            .join(format!("standard-{}.json", candidate.id))
            .to_string_lossy()
            .into_owned()]);
        let mut attempt = qualify_template(&about, template.clone(), &answers, &probe, &scratch);
        // One retry, and only for a failure that was about this machine.
        if let Ok(evidence) = &attempt {
            if is_environmental(evidence) {
                println!(
                    "  {} — retrying once",
                    evidence.failure().map_or("", |s| &s.step)
                );
                attempt = qualify_template(&about, template, &answers, &probe, &scratch);
            }
        }
        match attempt {
            Ok(evidence) => {
                let ok = evidence.passed;
                let environmental = is_environmental(&evidence);
                if let Err(error) = batch.record(&evidence) {
                    println!("  could not record: {}", error.message);
                }
                if ok {
                    passed += 1;
                    println!("  passed");
                } else if environmental {
                    environmental_failures += 1;
                    println!(
                        "  could not finish on this machine: {:?}",
                        evidence.failure().map(|step| &step.step)
                    );
                } else {
                    failed += 1;
                    println!(
                        "  FAILED at {:?}: {:?}",
                        evidence.failure().map(|step| &step.step),
                        evidence.failure().and_then(|step| step.detail.as_deref())
                    );
                }
            }
            Err(error) => {
                failed += 1;
                println!("  could not run: {}", error.message);
            }
        }
        // Keeping images makes a re-run fast, which matters while a batch is
        // being iterated on; removing them keeps a hundred apps from filling
        // the disk. Keep them when asked, but never past the point where the
        // machine is in trouble — a batch that fills somebody's drive is worse
        // than a slow one.
        let low_on_space = free_gigabytes().is_some_and(|free| free < 20);
        if !keep_images || low_on_space {
            if low_on_space && keep_images {
                println!("  low on disk, removing the images this run pulled");
            }
            for image in &ours {
                let _ = std::process::Command::new("docker")
                    .args(["image", "rm", "-f", image])
                    .output();
            }
        }
    }
    println!(
        "passed {passed}, failed {failed}, skipped {skipped}, \
         could not finish here {environmental_failures}"
    );
}
