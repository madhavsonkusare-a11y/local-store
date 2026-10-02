//! Display claims derived from reviewed offerings and current exact-input proof.
//! A catalog listing, lifecycle pass and agent content access are separate facts.
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct LaunchReadiness {
    pub offering_id: String,
    pub install_mode: &'static str,
    pub lifecycle_proven: bool,
    pub task_verified: bool,
    pub current_evidence: bool,
    pub task: Option<String>,
    pub recorded_at_unix: Option<u64>,
    pub agent_content_access: &'static str,
    pub prerequisites: Vec<String>,
    pub caution: Option<String>,
}

type ProofInput = (&'static str, &'static [u8], &'static [u8], &'static [u8]);
const PROOFS: &[ProofInput] = &[
    (
        "flatnotes",
        include_bytes!("../src/templates/flatnotes.json"),
        include_bytes!("../docs/evidence/flatnotes-managed-content-2026-09-30.json"),
        include_bytes!("../scripts/content-roundtrip-probe.mjs"),
    ),
    (
        "gitea",
        include_bytes!("../src/templates/gitea.json"),
        include_bytes!("../docs/evidence/gitea-managed-repository-2026-09-30.json"),
        include_bytes!("../scripts/gitea-repository-probe.mjs"),
    ),
    (
        "immich",
        include_bytes!("../src/templates/immich.json"),
        include_bytes!("../docs/evidence/immich-managed-photo-2026-09-30.json"),
        include_bytes!("../scripts/immich-photo-probe.mjs"),
    ),
    (
        "jellyfin",
        include_bytes!("../src/templates/jellyfin.json"),
        include_bytes!("../docs/evidence/jellyfin-managed-media-2026-09-30.json"),
        include_bytes!("../scripts/jellyfin-media-probe.mjs"),
    ),
    (
        "kanboard",
        include_bytes!("../src/templates/kanboard.json"),
        include_bytes!("../docs/evidence/kanboard-managed-task-2026-09-28.json"),
        include_bytes!("../scripts/kanboard-task-probe.mjs"),
    ),
    (
        "memos",
        include_bytes!("../src/recipes/memos.json"),
        include_bytes!("../docs/evidence/memos-managed-resource-2026-09-23.json"),
        include_bytes!("../scripts/memos-content-probe.mjs"),
    ),
    (
        "n8n",
        include_bytes!("../src/recipes/n8n.json"),
        include_bytes!("../docs/evidence/n8n-managed-workflow-2026-09-30.json"),
        include_bytes!("../scripts/n8n-workflow-probe.mjs"),
    ),
    (
        "privatebin",
        include_bytes!("../src/templates/privatebin.json"),
        include_bytes!("../docs/evidence/privatebin-managed-paste-2026-09-30.json"),
        include_bytes!("../scripts/privatebin-browser-probe.mjs"),
    ),
    (
        "uptime-kuma",
        include_bytes!("../src/recipes/uptime-kuma.json"),
        include_bytes!("../docs/evidence/uptime-kuma-managed-monitor-2026-09-30.json"),
        include_bytes!("../scripts/uptime-kuma-monitor-probe.mjs"),
    ),
    (
        "wordpress",
        include_bytes!("../src/templates/wordpress.json"),
        include_bytes!("../docs/evidence/wordpress-managed-post-2026-09-30.json"),
        include_bytes!("../scripts/wordpress-post-probe.mjs"),
    ),
];
const LEDGER: &str = include_str!("../catalog/v1-qualified-apps.json");
const REQUIRED_STEPS: &[&str] = &[
    "installs with one action",
    "survives a restart",
    "is usable first install",
    "is usable after restart",
    "reinstalls over data it kept",
    "is usable after a keep-data reinstall",
    "removes everything it created",
    "leaves other containers alone",
];
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn for_offering(id: &str) -> LaunchReadiness {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs());
    for_offering_at(id, now)
}
pub fn for_offerings(ids: &[String]) -> Vec<LaunchReadiness> {
    ids.iter().take(100).map(|id| for_offering(id)).collect()
}
fn for_offering_at(id: &str, now: u64) -> LaunchReadiness {
    let mut view = LaunchReadiness {
        offering_id: id.into(),
        install_mode: "discovery",
        lifecycle_proven: false,
        task_verified: false,
        current_evidence: false,
        task: None,
        recorded_at_unix: None,
        agent_content_access: crate::agent_provider_evidence::access(id, now),
        prerequisites: Vec::new(),
        caution: None,
    };
    let Some(offering) = crate::offerings::offering(id) else {
        return view;
    };
    match offering.plan_template(None).and_then(|template| {
        template
            .setup_review()
            .map_err(crate::error::AppError::invalid)
    }) {
        Ok(review) => {
            let required: Vec<_> = review
                .fields
                .iter()
                .filter(|field| field.required && !field.has_default)
                .collect();
            view.install_mode = if required.is_empty() {
                "zero_input"
            } else {
                "setup_assisted"
            };
            view.prerequisites = required
                .iter()
                .map(|field| {
                    if field.control == "folder" {
                        format!("Choose {} before installing.", field.label)
                    } else {
                        format!(
                            "Provide {} before installing{}.",
                            field.label,
                            if field.sensitive {
                                " (kept private)"
                            } else {
                                ""
                            }
                        )
                    }
                })
                .collect();
        }
        Err(_) => {
            view.install_mode = "setup_unavailable";
            view.caution = Some(
                "Setup requirements could not be verified. Installation is unavailable.".into(),
            );
        }
    }
    // Reviewed risk notes carry account, external service and hardware requirements
    // verbatim. Do not guess that an API key is optional or a GPU is available.
    let summary = offering.summary(None);
    if let Ok(summary) = summary {
        for note in summary.risk_notes {
            let lower = note.to_lowercase();
            if [
                "account",
                "api key",
                "gpu",
                "administrator",
                "first launch",
                "complete",
                "sign-in",
            ]
            .iter()
            .any(|word| lower.contains(word))
            {
                view.prerequisites.push(note);
            }
        }
    }
    let Some((_, manifest_bytes, evidence_bytes, probe_bytes)) =
        PROOFS.iter().find(|input| input.0 == id)
    else {
        view.caution = Some(
            "Reviewed installation recipe. A current Windows app-task proof has not been recorded."
                .into(),
        );
        return view;
    };
    let Some(evidence) = current_proof(id, manifest_bytes, evidence_bytes, probe_bytes, now) else {
        view.caution = Some(
            "The recorded proof is stale or its inputs changed. Reverification is required.".into(),
        );
        return view;
    };
    view.current_evidence = true;
    view.lifecycle_proven = true;
    view.task_verified = true;
    view.task = Some(evidence.first_use);
    view.recorded_at_unix = Some(evidence.recorded_at_unix);
    view
}
fn current_proof(
    id: &str,
    manifest_bytes: &[u8],
    evidence_bytes: &[u8],
    probe_bytes: &[u8],
    now: u64,
) -> Option<crate::qualification::Evidence> {
    let ledger: Value = serde_json::from_str(LEDGER).ok()?;
    let entry = ledger["apps"]
        .as_array()?
        .iter()
        .find(|entry| entry["id"].as_str() == Some(id))?;
    if entry["manifest_sha256"].as_str()? != digest(manifest_bytes)
        || entry["evidence_sha256"].as_str()? != digest(evidence_bytes)
    {
        return None;
    }
    let evidence: crate::qualification::Evidence = serde_json::from_slice(evidence_bytes).ok()?;
    let identity = evidence.identity.as_ref()?;
    let manifest: Value = serde_json::from_slice(manifest_bytes).ok()?;
    let offering = crate::offerings::offering(id)?;
    let (kind, adapter, locator, revision, source_day, image_day, requested) = match &offering {
        crate::offerings::Offering::Recipe(recipe) => (
            "reviewed recipe",
            "recipe".to_owned(),
            recipe.source_url.clone(),
            String::new(),
            recipe.verified_at.clone(),
            recipe.requirements.image_audit.checked_at.clone(),
            vec![recipe.image.clone()],
        ),
        crate::offerings::Offering::Template(template) => (
            "reviewed mapping",
            template.origin.importer.clone(),
            format!("{}#{}", template.origin.repository, template.origin.path),
            template.origin.revision.clone(),
            template.verified_at.clone(),
            template
                .requirements
                .images
                .iter()
                .map(|image| image.checked_at.clone())
                .min()?,
            template
                .requirements
                .images
                .iter()
                .map(|image| image.image.clone())
                .collect(),
        ),
    };
    let plan = offering.plan_template(None).ok()?.plan.to_compose().ok()?;
    if identity.plan_sha256 != digest(plan.as_bytes()) {
        return None;
    }
    let requested: std::collections::BTreeSet<_> = requested.iter().collect();
    let proven: std::collections::BTreeSet<_> = identity.requested_images.iter().collect();
    if manifest["id"].as_str() != Some(id)
        || identity.source_kind != kind
        || identity.source_adapter != adapter
        || identity.source_locator != locator
        || identity.source_revision != revision
        || identity.source_observed_on != source_day
        || identity.images_observed_on != image_day
        || requested != proven
        || identity.engine != crate::runtime::engine::EngineBinding::managed_wsl()
        || identity.level != "lifecycle_and_first_use"
        || identity.resolved_image_ids.is_empty()
        || identity.resolved_image_ids != evidence.image_ids
    {
        return None;
    }
    let task = entry["task"].as_str()?;
    let mut probe_material = task.as_bytes().to_vec();
    probe_material.extend_from_slice(b"\0script-source:");
    probe_material.extend_from_slice(probe_bytes);
    if !evidence.passed
        || evidence.app != id
        || evidence.first_use != task
        || evidence.recorded_at_unix > now
        || !evidence.is_current_at(identity, now)
        || identity.probe_sha256 != digest(&probe_material)
        || identity.host_os != "windows"
        || identity.host_arch != "x86_64"
        || identity.engine.endpoint != "wsl://local-store-engine-v1"
        || evidence.steps.iter().any(|step| !step.passed)
        || !REQUIRED_STEPS
            .iter()
            .all(|name| evidence.steps.iter().any(|step| step.step == *name))
    {
        return None;
    }
    Some(evidence)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_presence_never_claims_install_or_content_access() {
        let view = for_offering_at("not-reviewed", 1);
        assert_eq!(view.install_mode, "discovery");
        assert!(!view.task_verified);
        assert!(!view.lifecycle_proven);
        assert_eq!(view.agent_content_access, "unverified");
    }
    #[test]
    fn exact_proofs_expire_and_changes_are_refused() {
        for (id, manifest, evidence, probe) in PROOFS {
            let parsed: crate::qualification::Evidence = serde_json::from_slice(evidence).unwrap();
            assert!(
                current_proof(id, manifest, evidence, probe, parsed.recorded_at_unix).is_some(),
                "{id}"
            );
            assert!(current_proof(
                id,
                manifest,
                evidence,
                probe,
                parsed.recorded_at_unix + crate::qualification::Evidence::MAX_REUSE_AGE_SECS + 1
            )
            .is_none());
            assert!(
                current_proof(id, b"changed", evidence, probe, parsed.recorded_at_unix).is_none()
            );
            assert!(
                current_proof(id, manifest, evidence, b"changed", parsed.recorded_at_unix)
                    .is_none()
            );
        }
    }
}
