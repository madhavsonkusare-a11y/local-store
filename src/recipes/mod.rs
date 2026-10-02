use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub catalog_name: String,
    pub description: String,
    pub category: String,
    pub license: String,
    pub version: String,
    pub image: String,
    pub source_url: String,
    pub documentation_url: String,
    pub verified_at: String,
    pub launch_url: String,
    pub health_url: String,
    /// Port published on this computer. This is the one a user could be
    /// offered a choice about, and the one the launch address uses.
    pub host_port: u16,
    /// Port the image listens on inside its container. Fixed by the image;
    /// changing it would break the container, not relocate it.
    pub container_port: u16,
    pub requirements: RecipeRequirements,
    pub data_directories: Vec<String>,
    pub data_storage: String,
    pub risk_notes: Vec<String>,
    pub compose: String,
    /// Reviewed ceilings must agree with the literal Compose file we install.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_limits: Vec<crate::templates::ResourceLimit>,
}

/// Container image compatibility, not proof of a host-platform installation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeRequirements {
    pub docker_engine_os: String,
    pub compose_major: u8,
    pub local_storage_required: bool,
    pub container_platforms: Vec<String>,
    pub image_audit: ImageAudit,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageAudit {
    pub source_url: String,
    pub checked_at: String,
    pub index_digest: String,
}

const MEMOS: &str = include_str!("memos.json");
const N8N: &str = include_str!("n8n.json");
const UPTIME_KUMA: &str = include_str!("uptime-kuma.json");

pub fn reviewed_recipes() -> Vec<Recipe> {
    [MEMOS, N8N, UPTIME_KUMA]
        .into_iter()
        .map(|source| {
            let recipe: Recipe =
                serde_json::from_str(source).expect("reviewed recipe JSON must be valid");
            recipe.validate().expect("reviewed recipe must pass policy");
            recipe
        })
        .collect()
}

pub fn recipe(id: &str) -> Option<Recipe> {
    reviewed_recipes()
        .into_iter()
        .find(|recipe| recipe.id == id)
}

pub fn recipe_for_catalog_name(name: &str) -> Option<Recipe> {
    reviewed_recipes()
        .into_iter()
        .find(|recipe| recipe.catalog_name.eq_ignore_ascii_case(name))
}

impl Recipe {
    /// Republish this recipe on a different host port.
    ///
    /// Only the host side of the mapping moves: the container port belongs to
    /// the image. The launch and health addresses and the risk notes are
    /// carried along, and the result is validated, so a rewrite that left any
    /// of them behind is refused rather than installed.
    pub fn with_host_port(&self, host_port: u16) -> Result<Self, String> {
        self.validate()?;
        if host_port == self.host_port {
            return Ok(self.clone());
        }
        // Ports below 1024 need privileges the app does not ask for, and 0 asks
        // the kernel to choose, which nothing downstream could then address.
        if host_port < 1024 {
            return Err("Choose a port between 1024 and 65535.".into());
        }
        let published = format!("\"127.0.0.1:{}:{}\"", self.host_port, self.container_port);
        if self.compose.matches(published.as_str()).count() != 1 {
            return Err("This recipe does not publish exactly one port to remap.".into());
        }
        let moved = format!("\"127.0.0.1:{}:{}\"", host_port, self.container_port);
        let old = format!(":{}", self.host_port);
        let new = format!(":{}", host_port);
        let remapped = Self {
            host_port,
            compose: self.compose.replace(published.as_str(), &moved),
            launch_url: self.launch_url.replacen(&old, &new, 1),
            health_url: self.health_url.replacen(&old, &new, 1),
            // The notes state the published port; leaving them would describe
            // an app that is not the one about to be installed.
            risk_notes: self
                .risk_notes
                .iter()
                .map(|note| note.replace(&self.host_port.to_string(), &host_port.to_string()))
                .collect(),
            ..self.clone()
        };
        remapped.validate()?;
        Ok(remapped)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 3 {
            return Err("unsupported recipe schema".into());
        }
        if self.resource_limits.len() > 1 {
            return Err(
                "single-service recipes require exactly one reviewed resource ceiling".into(),
            );
        }
        if let Some(limit) = self.resource_limits.first() {
            if limit.service != self.id
                || !(16 * 1024 * 1024..=64 * 1024 * 1024 * 1024).contains(&limit.memory_bytes)
                || !(1..=64_000).contains(&limit.cpu_millicores)
                || !(1..=65_536).contains(&limit.pids)
                || limit.reason.trim().len() <= 20
            {
                return Err("invalid reviewed recipe resource ceiling".into());
            }
            let block = format!(
                "    mem_limit: {}\n    cpus: \"{}.{:03}\"\n    pids_limit: {}\n",
                limit.memory_bytes,
                limit.cpu_millicores / 1000,
                limit.cpu_millicores % 1000,
                limit.pids,
            );
            if self.compose.matches(&block).count() != 1
                || ["mem_limit:", "cpus:", "pids_limit:"]
                    .iter()
                    .any(|key| self.compose.matches(key).count() != 1)
            {
                return Err("recipe Compose differs from its reviewed resource ceiling".into());
            }
        } else if ["mem_limit:", "cpus:", "pids_limit:"]
            .iter()
            .any(|key| self.compose.contains(key))
        {
            return Err("recipe Compose has an unreviewed resource ceiling".into());
        }
        let requirements = &self.requirements;
        let audit = &requirements.image_audit;
        let digest = audit
            .index_digest
            .strip_prefix("sha256:")
            .unwrap_or_default();
        if requirements.docker_engine_os != "linux"
            || requirements.compose_major != 2
            || !requirements.local_storage_required
            || requirements.container_platforms.is_empty()
            || requirements.container_platforms.iter().any(|platform| {
                !matches!(
                    platform.as_str(),
                    "linux/amd64" | "linux/arm64" | "linux/arm/v7"
                )
            })
            || requirements
                .container_platforms
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || !valid_audit_date(&audit.checked_at)
            || !audit.source_url.starts_with("https://")
            || crate::windowing::validated_external_url(&audit.source_url).is_err()
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(
                "recipe requirements need supported Linux platforms and image audit evidence"
                    .into(),
            );
        }
        if !valid_audit_date(&self.verified_at)
            || !self.source_url.starts_with("https://")
            || !self.documentation_url.starts_with("https://")
        {
            return Err("recipe audit metadata is missing".into());
        }
        if !crate::model::is_valid_installed_app_id(&self.id)
            || self.display_name.trim().is_empty()
            || self.catalog_name.trim().is_empty()
            || self.description.trim().is_empty()
            || self.category.trim().is_empty()
            || self.license.trim().is_empty()
        {
            return Err("invalid recipe identity".into());
        }
        let expected_suffix = format!(":{}", self.version);
        if self.version.trim().is_empty()
            || !self.image.ends_with(&expected_suffix)
            || self.image.ends_with(":latest")
            || self.image.ends_with(":stable")
            || self.image.ends_with(":main")
            // By digest, not just by tag: the audited manifest list is what
            // installs, so a tag pushed again cannot change it.
            || !self.compose.contains(&format!(
                "image: {}@{}",
                self.image, self.requirements.image_audit.index_digest
            ))
            || !self.compose.contains(&format!(
                "\"127.0.0.1:{}:{}\"",
                self.host_port, self.container_port
            ))
        {
            return Err("recipe image and port must be pinned exactly".into());
        }
        // The addresses the launcher opens and probes must be the published
        // port, not the one inside the container. Getting this wrong would send
        // the user to a port nothing is listening on.
        if self.host_port < 1024
            || self.container_port == 0
            || !recipe_address_uses_port(&self.launch_url, self.host_port)
            || !recipe_address_uses_port(&self.health_url, self.host_port)
        {
            return Err("recipe launch and health addresses must use the published port".into());
        }
        if self.data_storage.trim().is_empty() {
            return Err("recipe data storage description is missing".into());
        }
        for directory in &self.data_directories {
            let path = Path::new(directory);
            if path.is_absolute()
                || directory.is_empty()
                || path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir | std::path::Component::RootDir
                    )
                })
            {
                return Err("recipe data directory escapes its managed project".into());
            }
        }
        for forbidden in [
            "privileged:",
            "/var/run/docker.sock",
            "network_mode: host",
            "pid: host",
            "ipc: host",
            "cap_add:",
            "devices:",
            "- /:/",
        ] {
            if self.compose.contains(forbidden) {
                return Err(format!("recipe contains forbidden setting: {forbidden}"));
            }
        }
        Ok(())
    }
}

fn recipe_address_uses_port(value: &str, port: u16) -> bool {
    if crate::windowing::validated_external_url(value).is_err() {
        return false;
    }
    let Ok(url) = tauri::Url::parse(value) else {
        return false;
    };
    url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        && url.port() == Some(port)
        && url.fragment().is_none()
}

fn valid_audit_date(value: &str) -> bool {
    if value.len() != 10 || !value.is_ascii() {
        return false;
    }
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    year >= 2020 && day > 0 && day <= days
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_recipes_are_pinned_persistent_and_unprivileged() {
        for recipe in reviewed_recipes() {
            recipe.validate().unwrap();
            assert!(recipe.compose.contains("./data:") || recipe.compose.contains("n8n-data:"));
        }
    }

    #[test]
    fn only_explicitly_reviewed_recipes_are_installable() {
        assert_eq!(reviewed_recipes().len(), 3);
        assert!(recipe("memos").is_some());
        assert!(recipe("n8n").is_some());
        assert!(recipe("uptime-kuma").is_some());
        assert!(recipe("immich").is_none());
    }

    #[test]
    fn unsafe_recipe_changes_fail_closed() {
        let mut candidate = recipe("memos").unwrap();
        candidate.compose.push_str("    privileged: true\n");
        assert!(candidate.validate().is_err());
        candidate = recipe("memos").unwrap();
        candidate.image = "neosmemo/memos:latest".into();
        assert!(candidate.validate().is_err());
        candidate = recipe("memos").unwrap();
        candidate.data_directories = vec!["../escape".into()];
        assert!(candidate.validate().is_err());
        candidate = recipe("memos").unwrap();
        candidate.compose = candidate.compose.replace("127.0.0.1:", "");
        assert!(candidate.validate().is_err());
    }

    #[test]
    fn reviewed_resource_ceilings_refuse_compose_and_service_drift() {
        let reviewed = recipe("memos").unwrap();
        assert_eq!(reviewed.resource_limits.len(), 1);
        let mut changed = reviewed.clone();
        changed.resource_limits[0].service = "other".into();
        assert!(changed.validate().is_err());
        changed = reviewed.clone();
        changed
            .resource_limits
            .push(changed.resource_limits[0].clone());
        assert!(changed.validate().is_err());
        changed = reviewed.clone();
        changed.resource_limits[0].memory_bytes += 1;
        assert!(changed.validate().is_err());
        changed = reviewed.clone();
        changed.compose.push_str("    mem_limit: 1\n");
        assert!(changed.validate().is_err());
        changed = reviewed;
        changed.resource_limits.clear();
        assert!(changed.validate().is_err());
    }

    #[test]
    fn a_manifest_that_disagrees_with_its_compose_is_refused() {
        // The published port is the one a user can be offered a choice about.
        // A manifest claiming a port the Compose file does not publish would
        // send the launcher, the port preflight and the user to nothing.
        let mut candidate = recipe("memos").unwrap();
        candidate.host_port = 8080;
        assert!(
            candidate.validate().is_err(),
            "a published port absent from the Compose mapping was accepted"
        );

        // The container port belongs to the image; swapping it silently breaks
        // the mapping rather than relocating the app.
        candidate = recipe("memos").unwrap();
        candidate.container_port = 9999;
        assert!(candidate.validate().is_err());

        // A genuine remap must carry the launch and health addresses with it.
        candidate = recipe("memos").unwrap();
        candidate.compose = candidate
            .compose
            .replace("\"127.0.0.1:5230:5230\"", "\"127.0.0.1:8080:5230\"");
        candidate.host_port = 8080;
        assert!(
            candidate.validate().is_err(),
            "a stale launch address was accepted after a remap"
        );
        candidate.launch_url = "http://localhost:8080".into();
        candidate.health_url = "http://localhost:8080".into();
        assert!(
            candidate.validate().is_ok(),
            "a consistent remap should be accepted: {:?}",
            candidate.validate()
        );
    }

    #[test]
    fn republishing_on_another_port_carries_every_address_with_it() {
        let original = recipe("memos").unwrap();
        let moved = original.with_host_port(8080).unwrap();

        assert_eq!(moved.host_port, 8080);
        // The container port belongs to the image and must not move.
        assert_eq!(moved.container_port, original.container_port);
        assert!(moved
            .compose
            .contains(&format!("\"127.0.0.1:8080:{}\"", original.container_port)));
        assert!(!moved.compose.contains("127.0.0.1:5230:"));
        assert_eq!(moved.launch_url, "http://localhost:8080");
        assert_eq!(moved.health_url, "http://localhost:8080");
        // The notes describe the app the user is about to install.
        assert!(moved.risk_notes.iter().any(|note| note.contains("8080")));
        assert!(!moved.risk_notes.iter().any(|note| note.contains("5230")));
        // The internal port is untouched, so the container still works.
        assert!(moved.compose.contains("MEMOS_PORT: \"5230\""));
        moved.validate().unwrap();
    }

    #[test]
    fn a_remap_that_cannot_be_made_consistent_is_refused() {
        let original = recipe("memos").unwrap();
        // Privileged ports need rights the app never asks for.
        assert!(original.with_host_port(80).is_err());
        assert!(original.with_host_port(0).is_err());
        // Asking for the port it already uses is a no-op, not an error.
        assert_eq!(
            original
                .with_host_port(original.host_port)
                .unwrap()
                .host_port,
            original.host_port
        );

        // If the mapping is not the single published one this recipe declares,
        // the rewrite has nothing safe to move and must not guess.
        let mut tampered = original.clone();
        tampered.compose = tampered
            .compose
            .replace("127.0.0.1:5230:5230", "0.0.0.0:5230:5230");
        assert!(tampered.with_host_port(8080).is_err());
    }

    #[test]
    fn an_older_manifest_schema_is_refused() {
        let mut candidate = recipe("memos").unwrap();
        for version in [1, 2] {
            candidate.schema_version = version;
            assert!(candidate.validate().is_err());
        }
    }

    #[test]
    fn platform_requirements_reject_unsupported_or_unaudited_claims() {
        let original = recipe("memos").unwrap();
        for platforms in [
            vec![],
            vec!["windows/amd64"],
            vec!["linux/amd64", "linux/amd64"],
        ] {
            let mut changed = original.clone();
            changed.requirements.container_platforms =
                platforms.into_iter().map(String::from).collect();
            assert!(changed.validate().is_err());
        }
        let mut changed = original.clone();
        changed.requirements.image_audit.index_digest = "sha256:1234".into();
        assert!(changed.validate().is_err());
        changed = original.clone();
        changed.requirements.compose_major = 1;
        assert!(changed.validate().is_err());
        changed = original.clone();
        changed.requirements.docker_engine_os = "windows".into();
        assert!(changed.validate().is_err());
        changed = original;
        changed.requirements.local_storage_required = false;
        assert!(changed.validate().is_err());
    }

    #[test]
    fn requirements_are_mandatory_and_survive_a_port_override() {
        let original = recipe("memos").unwrap();
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("requirements");
        assert!(serde_json::from_value::<Recipe>(json).is_err());
        assert_eq!(
            original.requirements,
            original.with_host_port(8080).unwrap().requirements
        );
    }

    #[test]
    fn audit_date_accepts_future_reviews_and_rejects_invalid_dates() {
        assert!(valid_audit_date("2026-10-01"));
        assert!(valid_audit_date("2028-02-29"));
        for invalid in ["", "2026-02-29", "2026-09-31", "2026-13-01", "2026-00-01"] {
            assert!(!valid_audit_date(invalid));
        }
    }

    #[test]
    fn address_validation_checks_the_actual_loopback_port_not_a_substring() {
        let original = recipe("memos").unwrap();
        for address in [
            "http://localhost:52300",
            "http://localhost:8080/path:5230",
            "http://example.org:5230",
            "http://user:secret@localhost:5230",
            "https://localhost:5230",
            "http://localhost:5230/#fragment",
        ] {
            let mut changed = original.clone();
            changed.health_url = address.into();
            assert!(changed.validate().is_err(), "accepted {address}");
            assert!(changed.with_host_port(changed.host_port).is_err());
        }
        let mut with_path = original;
        with_path.health_url = "http://localhost:5230/check:5230?port=5230".into();
        assert_eq!(
            with_path.with_host_port(8080).unwrap().health_url,
            "http://localhost:8080/check:5230?port=5230"
        );
    }
}
