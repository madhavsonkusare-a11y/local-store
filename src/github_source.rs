//! Resolve a GitHub repository identity before considering any deployment data.
//! A URL is never an install recipe. Only the reviewed offering allowlist can
//! turn a repository link into an immediately installable app.

use crate::offerings::{offerings, Offering};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GithubResolution {
    ApprovedMatch {
        repository: String,
        offering_id: String,
        display_name: String,
    },
    AmbiguousMatch {
        repository: String,
        offering_ids: Vec<String>,
    },
    NeedsReview {
        repository: String,
    },
    ReviewCandidates {
        repository: String,
        candidates: Vec<CandidateReference>,
    },
}

/// A pinned upstream-catalog definition, not an approved install recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandidateReference {
    pub source: String,
    pub id: String,
    pub revision: String,
    pub path: String,
    pub structurally_importable: bool,
}

#[derive(Deserialize)]
struct CandidateQueue {
    candidates: Vec<QueuedCandidate>,
}

#[derive(Deserialize)]
struct QueuedCandidate {
    source: String,
    id: String,
    identity: String,
    identity_status: String,
    importable: bool,
    provenance: CandidateProvenance,
}

#[derive(Deserialize)]
struct CandidateProvenance {
    revision: String,
    path: String,
}

fn reviewed_candidates(repository: &str) -> Result<Vec<CandidateReference>, String> {
    let queue: CandidateQueue =
        serde_json::from_str(include_str!("../catalog/candidate-queue.json"))
            .map_err(|_| "The pinned candidate index is unavailable.")?;
    let identity = repository.replacen("https://github.com/", "github:", 1);
    let mut matches = queue
        .candidates
        .into_iter()
        .filter(|row| {
            row.identity_status == "reviewed_repository_match" && row.identity == identity
        })
        .map(|row| CandidateReference {
            source: row.source,
            id: row.id,
            revision: row.provenance.revision,
            path: row.provenance.path,
            structurally_importable: row.importable,
        })
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| (&a.source, &a.id).cmp(&(&b.source, &b.id)));
    Ok(matches)
}

fn repository(url: &str) -> Result<String, String> {
    let parsed = Url::parse(url.trim()).map_err(|_| "Enter a valid GitHub repository URL.")?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Use an HTTPS github.com repository URL without credentials or a port.".into());
    }
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or("Enter a GitHub repository URL with an owner and repository.")?
        .filter(|segment| !segment.is_empty())
        .collect();
    let [owner, name, ..] = segments.as_slice() else {
        return Err("Enter a GitHub repository URL with an owner and repository.".into());
    };
    let name = name.strip_suffix(".git").unwrap_or(name);
    let plain = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            && part != "."
            && part != ".."
    };
    if !plain(owner) || !plain(name) {
        return Err("GitHub owner and repository must be plain names.".into());
    }
    Ok(format!(
        "https://github.com/{}/{}",
        owner.to_ascii_lowercase(),
        name.to_ascii_lowercase()
    ))
}

pub fn resolve(url: &str) -> Result<GithubResolution, String> {
    resolve_from(url, &offerings())
}

fn resolve_from(url: &str, approved: &[Offering]) -> Result<GithubResolution, String> {
    let repository = repository(url)?;
    let mut matches = approved
        .iter()
        .filter_map(|offering| {
            let source = offering.summary(None).ok()?.source_url;
            (self::repository(&source).ok().as_deref() == Some(repository.as_str()))
                .then(|| (offering.id().to_owned(), offering.display_name().to_owned()))
        })
        .collect::<Vec<_>>();
    matches.sort();
    Ok(match matches.as_slice() {
        [] => {
            let candidates = reviewed_candidates(&repository)?;
            if candidates.is_empty() {
                GithubResolution::NeedsReview { repository }
            } else {
                GithubResolution::ReviewCandidates {
                    repository,
                    candidates,
                }
            }
        }
        [(offering_id, display_name)] => GithubResolution::ApprovedMatch {
            repository,
            offering_id: offering_id.clone(),
            display_name: display_name.clone(),
        },
        _ => GithubResolution::AmbiguousMatch {
            repository,
            offering_ids: matches.into_iter().map(|(id, _)| id).collect(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_repository_paths_resolve_to_the_reviewed_app() {
        for url in [
            "https://github.com/usememos/memos",
            "https://github.com/usememos/memos.git",
            "https://github.com/UseMemos/Memos/tree/main/docs",
        ] {
            assert!(matches!(
                resolve(url).unwrap(),
                GithubResolution::ApprovedMatch { offering_id, .. } if offering_id == "memos"
            ));
        }
    }

    #[test]
    fn unknown_repository_stays_a_review_candidate() {
        assert_eq!(
            resolve("https://github.com/example/unreviewed").unwrap(),
            GithubResolution::NeedsReview {
                repository: "https://github.com/example/unreviewed".into()
            }
        );
    }

    #[test]
    fn reviewed_but_unapproved_repository_returns_pinned_definition_only() {
        let result = resolve("https://github.com/sissbruecker/linkding").unwrap();
        let GithubResolution::ReviewCandidates { candidates, .. } = result else {
            panic!("expected reviewed candidate");
        };
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, "caprover");
        assert_eq!(candidates[0].id, "linkding");
        assert_eq!(candidates[0].revision.len(), 40);
        assert!(candidates[0].structurally_importable);
    }

    #[test]
    fn lookalikes_and_non_repositories_are_refused() {
        for url in [
            "http://github.com/usememos/memos",
            "https://github.com.evil.test/usememos/memos",
            "https://user:secret@github.com/usememos/memos",
            "https://github.com:444/usememos/memos",
            "https://github.com/usememos",
            "https://github.com/usememos/%2fmemos",
            "https://github.com/../memos",
        ] {
            assert!(resolve(url).is_err(), "{url}");
        }
    }
}
