//! Read-only facts for the candidate promotion receipt. Uses the same approved
//! offering, plan and proof projection as the launcher; never imports or runs
//! a repository URL, promotes a candidate, or installs containers.
use local_store::{github_source, offerings::Offering};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let id = args
        .next()
        .ok_or("usage: github_review_facts <offering-id> [--inspect-github]")?;
    let inspect = match args.next().as_deref() {
        None => false,
        Some("--inspect-github") => true,
        _ => return Err("unsupported argument".into()),
    };
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let offering = local_store::offerings::offering(&id).ok_or("not an approved offering")?;
    let Offering::Template(reviewed) = &offering else {
        return Err("a candidate receipt requires an imported reviewed template".into());
    };
    let template = offering.plan_template(None)?;
    let plan_sha256 = format!(
        "{:x}",
        Sha256::digest(template.plan.to_compose()?.as_bytes())
    );
    let resolution = github_source::resolve(&reviewed.source_url)?;
    if !matches!(&resolution, github_source::GithubResolution::ApprovedMatch { offering_id, .. } if offering_id == &id)
    {
        return Err("the exact repository does not resolve uniquely to this offering".into());
    }
    let inspected = inspect
        .then(|| github_source::inspect(&reviewed.source_url))
        .transpose()?;
    if let Some(inspection) = &inspected {
        if !matches!(&inspection.resolution, github_source::GithubResolution::ApprovedMatch { offering_id, .. } if offering_id == &id)
        {
            return Err("the live canonical repository does not resolve to this offering".into());
        }
    }
    let readiness = local_store::launch_readiness::for_offering(&id);
    if !readiness.current_evidence || !readiness.task_verified || !readiness.lifecycle_proven {
        return Err("current exact-input Windows task evidence is required".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 1,
            "observed_at_unix": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            "app": id,
            "plan_sha256": plan_sha256,
            "imported_definition_sha256": format!("{:x}", Sha256::digest(reviewed.definition.as_bytes())),
            "source_resolution": resolution,
            "github_inspection": inspected,
            "launch_readiness": readiness,
        }))?
    );
    Ok(())
}
