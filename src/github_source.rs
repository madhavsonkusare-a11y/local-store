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

// Live inspection never returns an install plan. It pins a public GitHub
// repository identity and current default-branch commit before local matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GithubInspection {
    pub requested_repository: String,
    pub canonical_repository: String,
    pub repository_id: u64,
    pub default_branch: String,
    pub commit_sha: String,
    pub commit_url: String,
    pub archived: bool,
    pub resolution: GithubResolution,
}

const MAX_RESPONSE_BYTES: u64 = 128 * 1024;
const MAX_REDIRECTS: usize = 3;

struct HttpReply {
    status: u16,
    location: Option<String>,
    body: Vec<u8>,
}

trait HttpTransport {
    fn get(&self, url: &Url) -> Result<HttpReply, String>;
}

struct GithubHttp {
    client: reqwest::blocking::Client,
}

impl GithubHttp {
    fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Local-Store-repository-inspection")
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Could not initialize GitHub inspection.")?;
        Ok(Self { client })
    }
}

impl HttpTransport for GithubHttp {
    fn get(&self, url: &Url) -> Result<HttpReply, String> {
        use std::io::Read;
        let response = self
            .client
            .get(url.clone())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .map_err(|_| "GitHub metadata request failed.")?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        response
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Could not read GitHub metadata.")?;
        if body.len() as u64 > MAX_RESPONSE_BYTES {
            return Err("GitHub metadata response was too large.".into());
        }
        Ok(HttpReply {
            status,
            location,
            body,
        })
    }
}

#[derive(Deserialize)]
struct GithubRepository {
    id: u64,
    full_name: String,
    html_url: String,
    default_branch: String,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    private: bool,
}

#[derive(Deserialize)]
struct GithubRef {
    #[serde(rename = "ref")]
    reference: String,
    object: GithubRefObject,
}

#[derive(Deserialize)]
struct GithubRefObject {
    #[serde(rename = "type")]
    kind: String,
    sha: String,
}

fn api_repository_url(repository: &str) -> Result<Url, String> {
    Url::parse(&repository.replacen("https://github.com/", "https://api.github.com/repos/", 1))
        .map_err(|_| "Could not construct GitHub metadata request.".into())
}

enum ApiRepositoryTarget {
    Named(String),
    Id(u64),
}

fn api_repository_target(url: &Url) -> Result<ApiRepositoryTarget, String> {
    if url.scheme() != "https"
        || url.host_str() != Some("api.github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("GitHub redirected metadata outside its repository API.".into());
    }
    let parts: Vec<_> = url
        .path_segments()
        .ok_or("GitHub returned an invalid repository API URL.")?
        .collect();
    match parts.as_slice() {
        ["repos", owner, name] => {
            let repo = repository(&format!("https://github.com/{owner}/{name}"))?;
            if api_repository_url(&repo)?.path() != url.path() {
                return Err("GitHub redirected metadata to a noncanonical path.".into());
            }
            Ok(ApiRepositoryTarget::Named(repo))
        }
        ["repositories", id]
            if !id.starts_with('0')
                && !id.is_empty()
                && id.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            let id = id
                .parse::<u64>()
                .map_err(|_| "GitHub returned an invalid repository ID redirect.")?;
            Ok(ApiRepositoryTarget::Id(id))
        }
        _ => Err("GitHub redirected metadata outside its repository API.".into()),
    }
}

fn redirected_repository_url(current: &Url, location: &str) -> Result<Url, String> {
    let next = current
        .join(location)
        .map_err(|_| "GitHub returned an invalid repository redirect.")?;
    api_repository_target(&next)?;
    Ok(next)
}

fn fetch_repository<T: HttpTransport>(
    transport: &T,
    requested: &str,
) -> Result<GithubRepository, String> {
    let mut url = api_repository_url(requested)?;
    for attempt in 0..=MAX_REDIRECTS {
        let reply = transport.get(&url)?;
        match reply.status {
            200 => {
                let metadata: GithubRepository = serde_json::from_slice(&reply.body)
                    .map_err(|_| "GitHub returned malformed repository metadata.")?;
                let canonical = repository(&metadata.html_url)?;
                let public_url = Url::parse(&metadata.html_url)
                    .map_err(|_| "GitHub returned an invalid public repository URL.")?;
                let full_name = canonical.strip_prefix("https://github.com/").unwrap();
                let target_matches = match api_repository_target(&url)? {
                    ApiRepositoryTarget::Named(name) => canonical == name,
                    ApiRepositoryTarget::Id(id) => metadata.id == id,
                };
                if !target_matches
                    || public_url.query().is_some()
                    || public_url.fragment().is_some()
                    || public_url.path().to_ascii_lowercase() != format!("/{full_name}")
                    || !metadata.full_name.eq_ignore_ascii_case(full_name)
                    || metadata.id == 0
                    || metadata.private
                {
                    return Err("GitHub repository identity could not be verified.".into());
                }
                return Ok(metadata);
            }
            301 | 302 | 307 | 308 if attempt < MAX_REDIRECTS => {
                let location = reply
                    .location
                    .ok_or("GitHub omitted the repository redirect target.")?;
                url = redirected_repository_url(&url, &location)?;
            }
            301 | 302 | 307 | 308 => {
                return Err("GitHub redirected the repository too many times.".into());
            }
            404 => return Err("GitHub repository is unavailable or private.".into()),
            403 | 429 => return Err("GitHub rate-limited repository inspection; try later.".into()),
            _ => return Err("GitHub could not verify this repository.".into()),
        }
    }
    Err("GitHub redirected the repository too many times.".into())
}

fn ref_url(repository: &str, branch: &str) -> Result<Url, String> {
    if branch.is_empty()
        || branch.len() > 255
        || branch.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
    {
        return Err("GitHub returned an unsupported default branch name.".into());
    }
    let mut url = api_repository_url(repository)?;
    url.path_segments_mut()
        .map_err(|_| "Could not construct GitHub commit request.")?
        .push("git")
        .push("ref")
        .push("heads");
    for part in branch.split('/') {
        url.path_segments_mut()
            .map_err(|_| "Could not construct GitHub commit request.")?
            .push(part);
    }
    Ok(url)
}

fn fetch_commit<T: HttpTransport>(
    transport: &T,
    repository: &str,
    branch: &str,
) -> Result<String, String> {
    let reply = transport.get(&ref_url(repository, branch)?)?;
    if reply.status != 200 {
        return Err("GitHub could not pin the default-branch commit.".into());
    }
    let git_ref: GithubRef = serde_json::from_slice(&reply.body)
        .map_err(|_| "GitHub returned malformed commit metadata.")?;
    if git_ref.reference != format!("refs/heads/{branch}")
        || git_ref.object.kind != "commit"
        || git_ref.object.sha.len() != 40
        || !git_ref
            .object
            .sha
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("GitHub returned an invalid commit identity.".into());
    }
    Ok(git_ref.object.sha.to_ascii_lowercase())
}

pub fn inspect(url: &str) -> Result<GithubInspection, String> {
    inspect_with(url, &GithubHttp::new()?)
}

fn inspect_with<T: HttpTransport>(url: &str, transport: &T) -> Result<GithubInspection, String> {
    let requested_repository = repository(url)?;
    let metadata = fetch_repository(transport, &requested_repository)?;
    let canonical_repository = repository(&metadata.html_url)?;
    let commit_sha = fetch_commit(transport, &canonical_repository, &metadata.default_branch)?;
    let commit_url = format!("{canonical_repository}/commit/{commit_sha}");
    // Match only the canonical identity returned by GitHub. A renamed URL
    // must never inherit an approval or reviewed definition from an alias.
    let resolution = resolve(&canonical_repository)?;
    Ok(GithubInspection {
        requested_repository,
        canonical_repository,
        repository_id: metadata.id,
        default_branch: metadata.default_branch,
        commit_sha,
        commit_url,
        archived: metadata.archived,
        resolution,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeHttp(Mutex<VecDeque<(String, HttpReply)>>);

    impl HttpTransport for FakeHttp {
        fn get(&self, url: &Url) -> Result<HttpReply, String> {
            let (expected, reply) = self
                .0
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected GitHub request");
            assert_eq!(url.as_str(), expected);
            Ok(reply)
        }
    }

    fn fake_http(replies: Vec<(&str, HttpReply)>) -> FakeHttp {
        FakeHttp(Mutex::new(
            replies
                .into_iter()
                .map(|(url, reply)| (url.to_owned(), reply))
                .collect(),
        ))
    }

    fn reply(status: u16, location: Option<&str>, body: serde_json::Value) -> HttpReply {
        HttpReply {
            status,
            location: location.map(str::to_owned),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    fn repo_body(full_name: &str, branch: &str) -> serde_json::Value {
        serde_json::json!({
            "id": 42,
            "full_name": full_name,
            "html_url": format!("https://github.com/{full_name}"),
            "default_branch": branch,
            "archived": false,
            "private": false
        })
    }

    fn commit_body(sha: &str) -> serde_json::Value {
        serde_json::json!({"ref": "refs/heads/main", "object": {"type": "commit", "sha": sha}})
    }

    #[test]
    fn inspection_pins_canonical_approved_repo_and_commit() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let http = fake_http(vec![
            (
                "https://api.github.com/repos/usememos/memos",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
            (
                "https://api.github.com/repos/usememos/memos/git/ref/heads/main",
                reply(200, None, commit_body(sha)),
            ),
        ]);
        let result = inspect_with("https://github.com/UseMemos/Memos/tree/main", &http).unwrap();
        assert_eq!(
            result.requested_repository,
            "https://github.com/usememos/memos"
        );
        assert_eq!(result.canonical_repository, result.requested_repository);
        assert_eq!(result.commit_sha, sha);
        assert_eq!(
            result.commit_url,
            format!("https://github.com/usememos/memos/commit/{sha}")
        );
        assert!(matches!(result.resolution,
            GithubResolution::ApprovedMatch { offering_id, .. } if offering_id == "memos"));
        assert!(http.0.lock().unwrap().is_empty());
    }

    #[test]
    fn renamed_alias_resolves_only_against_final_github_identity() {
        let sha = "abcdef0123456789abcdef0123456789abcdef01";
        let http = fake_http(vec![
            (
                "https://api.github.com/repos/old/memos",
                reply(
                    301,
                    Some("https://api.github.com/repos/usememos/memos"),
                    serde_json::json!({}),
                ),
            ),
            (
                "https://api.github.com/repos/usememos/memos",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
            (
                "https://api.github.com/repos/usememos/memos/git/ref/heads/main",
                reply(200, None, commit_body(sha)),
            ),
        ]);
        let result = inspect_with("https://github.com/old/memos", &http).unwrap();
        assert_eq!(result.requested_repository, "https://github.com/old/memos");
        assert_eq!(
            result.canonical_repository,
            "https://github.com/usememos/memos"
        );
        assert!(matches!(result.resolution,
            GithubResolution::ApprovedMatch { offering_id, .. } if offering_id == "memos"));
    }

    #[test]
    fn numeric_repository_redirect_requires_matching_id() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let http = fake_http(vec![
            (
                "https://api.github.com/repos/old/memos",
                reply(
                    301,
                    Some("https://api.github.com/repositories/42"),
                    serde_json::json!({}),
                ),
            ),
            (
                "https://api.github.com/repositories/42",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
            (
                "https://api.github.com/repos/usememos/memos/git/ref/heads/main",
                reply(200, None, commit_body(sha)),
            ),
        ]);
        let result = inspect_with("https://github.com/old/memos", &http).unwrap();
        assert_eq!(result.repository_id, 42);
        assert_eq!(
            result.canonical_repository,
            "https://github.com/usememos/memos"
        );

        let http = fake_http(vec![
            (
                "https://api.github.com/repos/old/memos",
                reply(
                    301,
                    Some("https://api.github.com/repositories/43"),
                    serde_json::json!({}),
                ),
            ),
            (
                "https://api.github.com/repositories/43",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
        ]);
        assert!(inspect_with("https://github.com/old/memos", &http).is_err());
    }

    #[test]
    fn approved_alias_moved_elsewhere_loses_its_approval() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let http = fake_http(vec![
            (
                "https://api.github.com/repos/usememos/memos",
                reply(
                    301,
                    Some("https://api.github.com/repos/example/unreviewed"),
                    serde_json::json!({}),
                ),
            ),
            (
                "https://api.github.com/repos/example/unreviewed",
                reply(200, None, repo_body("example/unreviewed", "main")),
            ),
            (
                "https://api.github.com/repos/example/unreviewed/git/ref/heads/main",
                reply(200, None, commit_body(sha)),
            ),
        ]);
        let result = inspect_with("https://github.com/usememos/memos", &http).unwrap();
        assert_eq!(
            result.canonical_repository,
            "https://github.com/example/unreviewed"
        );
        assert!(matches!(
            result.resolution,
            GithubResolution::NeedsReview { .. }
        ));
    }

    #[test]
    fn mismatched_default_branch_ref_is_refused() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let http = fake_http(vec![
            (
                "https://api.github.com/repos/usememos/memos",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
            (
                "https://api.github.com/repos/usememos/memos/git/ref/heads/main",
                reply(
                    200,
                    None,
                    serde_json::json!({
                        "ref": "refs/heads/other",
                        "object": {"type": "commit", "sha": sha}
                    }),
                ),
            ),
        ]);
        assert!(inspect_with("https://github.com/usememos/memos", &http).is_err());
    }
    #[test]
    fn redirects_cannot_escape_the_exact_repository_api() {
        for location in [
            "https://evil.example/repos/usememos/memos",
            "http://api.github.com/repos/usememos/memos",
            "https://api.github.com:444/repos/usememos/memos",
            "https://api.github.com/repos/usememos/memos/contents/compose.yaml",
            "https://api.github.com/repos/usememos/%2fmemos",
        ] {
            let http = fake_http(vec![(
                "https://api.github.com/repos/usememos/memos",
                reply(301, Some(location), serde_json::json!({})),
            )]);
            assert!(
                inspect_with("https://github.com/usememos/memos", &http).is_err(),
                "{location}"
            );
        }
    }

    #[test]
    fn commit_ref_redirect_and_non_commit_are_refused() {
        let http = fake_http(vec![
            (
                "https://api.github.com/repos/usememos/memos",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
            (
                "https://api.github.com/repos/usememos/memos/git/ref/heads/main",
                reply(
                    302,
                    Some("https://evil.example/commit"),
                    serde_json::json!({}),
                ),
            ),
        ]);
        assert!(inspect_with("https://github.com/usememos/memos", &http).is_err());

        let http = fake_http(vec![
            (
                "https://api.github.com/repos/usememos/memos",
                reply(200, None, repo_body("usememos/memos", "main")),
            ),
            (
                "https://api.github.com/repos/usememos/memos/git/ref/heads/main",
                reply(
                    200,
                    None,
                    serde_json::json!({"ref": "refs/heads/main", "object": {"type": "tag",
                    "sha": "0123456789abcdef0123456789abcdef01234567"}}),
                ),
            ),
        ]);
        assert!(inspect_with("https://github.com/usememos/memos", &http).is_err());
    }

    #[test]
    fn metadata_identity_mismatch_is_refused_before_commit_request() {
        let http = fake_http(vec![(
            "https://api.github.com/repos/usememos/memos",
            reply(200, None, repo_body("attacker/repo", "main")),
        )]);
        assert!(inspect_with("https://github.com/usememos/memos", &http).is_err());
    }
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
