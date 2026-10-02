//! Current-source provider proof, separate from installation and live permission.
use serde_json::Value;
use sha2::{Digest, Sha256};

type Inputs = Vec<(&'static str, &'static [u8])>;
fn inputs(id: &str) -> Option<(&'static [u8], Inputs)> {
    let boundary = include_bytes!("agent_content.rs").as_slice();
    let mut hashes = vec![("content_boundary_sha256", boundary)];
    let receipt: &[u8] = match id {
        "memos" => {
            hashes = vec![
                ("provider_sha256", boundary),
                (
                    "test_sha256",
                    include_bytes!("../tests/managed_agent_content.rs"),
                ),
            ];
            include_bytes!("../docs/evidence/memos-agent-content-2026-10-01.json")
        }
        "flatnotes" => {
            hashes = vec![
                ("provider_sha256", include_bytes!("agent_data.rs")),
                ("test_sha256", include_bytes!("agent_data/real_test.rs")),
            ];
            include_bytes!("../docs/evidence/flatnotes-agent-files-2026-10-02.json")
        }
        "n8n" => {
            hashes.extend([
                (
                    "provider_sha256",
                    include_bytes!("agent_content/n8n.rs").as_slice(),
                ),
                (
                    "test_sha256",
                    include_bytes!("../tests/managed_n8n_content.rs").as_slice(),
                ),
            ]);
            include_bytes!("../docs/evidence/n8n-agent-content-2026-10-02.json")
        }
        "privatebin" => {
            hashes.extend([
                (
                    "provider_sha256",
                    include_bytes!("agent_content/privatebin.rs").as_slice(),
                ),
                (
                    "test_sha256",
                    include_bytes!("../tests/managed_privatebin_content.rs").as_slice(),
                ),
                (
                    "runner_sha256",
                    include_bytes!("../providers/privatebin/runner.mjs").as_slice(),
                ),
                (
                    "profile_sha256",
                    include_bytes!("../providers/privatebin/seccomp_profile.json").as_slice(),
                ),
            ]);
            include_bytes!("../docs/evidence/privatebin-agent-content-2026-10-02.json")
        }
        _ => {
            hashes.extend([
                (
                    "provider_sha256",
                    include_bytes!("agent_content/app_api.rs").as_slice(),
                ),
                (
                    "test_sha256",
                    include_bytes!("../tests/managed_app_api_content.rs").as_slice(),
                ),
                (
                    "fixture_sha256",
                    include_bytes!("../scripts/app-api-read-fixture.mjs").as_slice(),
                ),
            ]);
            match id {
                "gitea" => {
                    include_bytes!("../docs/evidence/gitea-agent-api-content-2026-10-02.json")
                }
                "immich" => {
                    include_bytes!("../docs/evidence/immich-agent-api-content-2026-10-02.json")
                }
                "jellyfin" => {
                    include_bytes!("../docs/evidence/jellyfin-agent-api-content-2026-10-02.json")
                }
                "kanboard" => {
                    include_bytes!("../docs/evidence/kanboard-agent-api-content-2026-10-02.json")
                }
                "wordpress" => {
                    include_bytes!("../docs/evidence/wordpress-agent-api-content-2026-10-02.json")
                }
                "uptime-kuma" => {
                    include_bytes!("../docs/evidence/uptime-kuma-agent-api-content-2026-10-02.json")
                }
                _ => return None,
            }
        }
    };
    Some((receipt, hashes))
}

fn current(id: &str, receipt: &Value, hashes: &Inputs, now: u64) -> bool {
    let Some(observed) = receipt["recorded_at_unix"].as_u64() else {
        return false;
    };
    if receipt["schema_version"].as_u64() != Some(1)
        || receipt["passed"] != true
        || receipt.get("failure") != Some(&Value::Null)
        || observed > now.saturating_add(300)
        || now.saturating_sub(observed) > 30 * 86400
        || !receipt["steps"]
            .as_array()
            .is_some_and(|steps| steps.len() >= 4 && steps.iter().all(Value::is_string))
        || hashes.iter().any(|(field, bytes)| {
            receipt[*field].as_str() != Some(format!("{:x}", Sha256::digest(bytes)).as_str())
        })
    {
        return false;
    }
    if id == "flatnotes" {
        receipt["proof"] == "flatnotes-scoped-markdown-files"
            && [
                "owned_cleanup",
                "bystanders_preserved",
                "native_engine_files_unchanged",
            ]
            .iter()
            .all(|field| receipt[*field] == true)
    } else {
        receipt["app"] == id
            && receipt["engine"]["schema_version"].as_u64() == Some(2)
            && receipt["engine"]["program"] == "wsl.exe"
            && receipt["engine"]["endpoint"] == "wsl://local-store-engine-v1"
            && match id {
                "memos" | "n8n" => true,
                "privatebin" => receipt["browser_cleanup_passed"] == true,
                _ => receipt["cleanup_passed"] == true && receipt["bystanders_unchanged"] == true,
            }
    }
}

/// Demonstrated capability only. This never enrolls, logs in or grants a client.
pub(crate) fn access(id: &str, now: u64) -> &'static str {
    let Some((bytes, hashes)) = inputs(id) else {
        return "unverified";
    };
    let Ok(receipt) = serde_json::from_slice::<Value>(bytes) else {
        return "unverified";
    };
    if !current(id, &receipt, &hashes, now) {
        return "unverified";
    }
    if matches!(id, "memos" | "n8n" | "privatebin") {
        "verified_read_write"
    } else {
        "verified_read"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_launch_path_has_current_source_proof_without_claiming_live_permission() {
        for id in [
            "memos",
            "n8n",
            "privatebin",
            "flatnotes",
            "gitea",
            "immich",
            "jellyfin",
            "kanboard",
            "wordpress",
            "uptime-kuma",
        ] {
            let (bytes, _) = inputs(id).unwrap();
            let receipt: Value = serde_json::from_slice(bytes).unwrap();
            let observed = receipt["recorded_at_unix"].as_u64().unwrap();
            assert_eq!(
                access(id, observed),
                if matches!(id, "memos" | "n8n" | "privatebin") {
                    "verified_read_write"
                } else {
                    "verified_read"
                },
                "{id}"
            );
            assert_eq!(access(id, observed + 31 * 86400), "unverified", "{id}");
        }
    }
    #[test]
    fn current_proof_expires_and_changed_inputs_wrong_engine_or_failure_refuse_claims() {
        let (bytes, mut hashes) = inputs("memos").unwrap();
        let mut receipt: Value = serde_json::from_slice(bytes).unwrap();
        let observed = receipt["recorded_at_unix"].as_u64().unwrap();
        assert!(current("memos", &receipt, &hashes, observed));
        assert!(!current(
            "memos",
            &receipt,
            &hashes,
            observed + 30 * 86400 + 1
        ));
        assert!(!current(
            "memos",
            &receipt,
            &hashes,
            observed.saturating_sub(301)
        ));
        receipt["engine"]["program"] = Value::String("docker".into());
        assert!(!current("memos", &receipt, &hashes, observed));
        receipt = serde_json::from_slice(bytes).unwrap();
        receipt["passed"] = Value::Bool(false);
        assert!(!current("memos", &receipt, &hashes, observed));
        receipt = serde_json::from_slice(bytes).unwrap();
        hashes[0].1 = b"changed provider";
        assert!(!current("memos", &receipt, &hashes, observed));
        assert_eq!(access("unknown", observed), "unverified");
    }
}
