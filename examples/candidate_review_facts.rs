//! Read-only facts for a withheld candidate. These facts do not approve or
//! register an app and contain no setup answers or executable repository data.
use local_store::{github_source, templates::ReviewedTemplate};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let id = args
        .next()
        .ok_or("usage: candidate_review_facts <candidate-id>")?;
    if args.next().is_some()
        || id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || id.starts_with('-')
    {
        return Err("one plain candidate ID is required".into());
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("catalog/promotion-proposals")
        .join(format!("{id}-withheld.json"));
    let bytes = std::fs::read(path)?;
    if bytes.len() > 1024 * 1024 {
        return Err("candidate proposal exceeds its bound".into());
    }
    let reviewed: ReviewedTemplate = serde_json::from_slice(&bytes)?;
    if reviewed.id != id || reviewed.offerable() || local_store::offerings::offering(&id).is_some()
    {
        return Err("an unregistered withheld candidate is required".into());
    }
    let template = reviewed.plan_template()?;
    let inspection = github_source::inspect(&reviewed.source_url)?;
    let github_source::GithubResolution::ReviewCandidates { candidates, .. } =
        &inspection.resolution
    else {
        return Err("the public repository must resolve to pinned review candidates".into());
    };
    let matches = candidates
        .iter()
        .filter(|candidate| {
            candidate.id == id
                && candidate.source == reviewed.origin.importer
                && candidate.path == reviewed.origin.path
                && candidate.revision == reviewed.origin.revision
                && candidate.structurally_importable
        })
        .count();
    if inspection.archived || matches != 1 {
        return Err("exact live canonical repository and source candidate required".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 1,
            "observed_at_unix": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "app": id,
            "installable": false,
            "proposal_sha256": format!("{:x}", Sha256::digest(&bytes)),
            "plan_sha256": format!("{:x}", Sha256::digest(template.plan.to_compose()?.as_bytes())),
            "github_inspection": inspection,
        }))?
    );
    Ok(())
}
