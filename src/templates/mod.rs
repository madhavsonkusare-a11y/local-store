//! Imported definitions somebody has read and taken responsibility for.
//!
//! An importer decides what a definition *could* become. This module decides
//! what one is allowed to be. The difference matters most for setup fields:
//! `caprover::setup_variable` marks every field sensitive, because an upstream
//! catalogue has no way to say which of its variables is a credential, and
//! masking a time zone is a smaller mistake than showing an API key. A review
//! is where somebody who read the app says which is which.
//!
//! Nothing here is offered to anyone. A reviewed template is a template a
//! person could be shown; whether it appears in the catalog is a separate
//! decision, made by the project owner, and this module does not make it.
use crate::{plan::PlanMount, setup::PlanTemplate};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Where a definition came from, precisely enough to fetch it again.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplateOrigin {
    /// Which importer maps this format. The only mapping path there is.
    pub importer: String,
    pub repository: String,
    /// The commit the definition was taken from, not a branch.
    pub revision: String,
    pub path: String,
    /// The upstream project's licence, which governs reuse of the definition.
    pub license: String,
}

/// What a review concluded. A review that can only say yes is not a review,
/// so the outcome is recorded either way and the reason travels with it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplatePromotion {
    /// `approved` or `withheld`. Only an approved template may be offered.
    pub state: String,
    /// Why, in terms somebody could disagree with.
    pub reason: String,
}

/// One image the plan runs, and what was checked about it.
///
/// A recipe audits a single image. A multi-service template cannot: it is only
/// as portable as its least portable image and only as current as its oldest,
/// so each one is recorded separately.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplateImageAudit {
    pub image: String,
    pub source_url: String,
    pub checked_at: String,
    /// When the registry last rebuilt this tag. An old date is not a fault by
    /// itself, but it is the fact a promotion decision turns on most often.
    pub last_updated: String,
    pub container_platforms: Vec<String>,
    /// Per-architecture digests, as the registry lists them for the tag.
    pub digests: BTreeMap<String, String>,
    /// The manifest list those architectures belong to. Installing by this
    /// pins every architecture at once and leaves Docker to pick the one the
    /// engine runs, so a tag pushed again later cannot change what installs.
    /// Required for anything offered.
    #[serde(default)]
    pub index_digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplateRequirements {
    pub docker_engine_os: String,
    pub compose_major: u8,
    pub local_storage_required: bool,
    pub images: Vec<TemplateImageAudit>,
}

/// What a review decided about one setup field.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FieldReview {
    /// What to call it, in this project's words rather than upstream's.
    pub label: String,
    /// Whether this answer is a credential. The importer says yes to
    /// everything; saying no here is a decision somebody made after reading
    /// what the app does with the value.
    pub sensitive: bool,
}

/// A tag this review runs instead of the one its definition names.
///
/// Upstream catalogues lag upstream projects. When a definition is pinned to a
/// release that has since been superseded — and the newer one carries fixes
/// worth having — the choice used to be to offer the old version or nothing,
/// because the definition is kept verbatim and there was no way to say
/// otherwise. This is that way, and it is deliberately narrow: the same
/// repository, a different tag, and a reason somebody could disagree with.
///
/// It cannot point at a different image. A review that could swap
/// `nodered/node-red` for something else would not be a pin, it would be a
/// second definition wearing the first one's provenance.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImagePin {
    /// The image as the upstream definition writes it.
    pub definition: String,
    /// The image this review runs instead.
    pub replacement: String,
    pub reason: String,
}

impl ImagePin {
    /// Repository and tag, or an error naming which half is missing.
    fn split(image: &str) -> Result<(&str, &str), String> {
        image
            .rsplit_once(':')
            .filter(|(repository, tag)| !repository.is_empty() && !tag.is_empty())
            .ok_or_else(|| format!("image {image:?} names no tag"))
    }

    fn check(&self) -> Result<(), String> {
        let (from, from_tag) = Self::split(&self.definition)?;
        let (to, to_tag) = Self::split(&self.replacement)?;
        if from != to {
            return Err(format!(
                "an image pin may change a tag, not the image: {from:?} to {to:?}"
            ));
        }
        if from_tag == to_tag {
            return Err(format!("image pin for {from:?} changes nothing"));
        }
        if self.reason.len() <= 20 {
            return Err(format!("image pin for {from:?} gives no reason"));
        }
        Ok(())
    }
}

/// A reviewed replacement for one upstream app-data bind mount. Databases
/// which require POSIX ownership cannot initialize on a Windows-backed WSL
/// mount; a Docker named volume keeps their data on the engine's Linux disk.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StoragePin {
    pub service: String,
    pub source: String,
    pub target: String,
    pub volume: String,
    pub reason: String,
}

/// An app-specific ceiling selected after observing a real managed-engine run.
/// The service must still exist in the pinned upstream definition.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResourceLimit {
    pub service: String,
    pub memory_bytes: u64,
    pub cpu_millicores: u32,
    pub pids: u32,
    pub reason: String,
}

/// Companion config from the same repository and commit as the definition.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplateConfig {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReviewedTemplate {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub catalog_name: String,
    pub description: String,
    pub category: String,
    /// The application's own licence, which is not the definition's.
    pub license: String,
    pub source_url: String,
    pub documentation_url: String,
    pub verified_at: String,
    /// The version of the app itself, when its own definition names one.
    /// Dify's front door is an nginx, so the tag on the image a plan
    /// publishes is nginx's version, not Dify's.
    #[serde(default)]
    pub version: Option<String>,
    /// The recorded run that proves this template installs and survives a
    /// reinstall. A template without one has not been verified, whatever else
    /// it declares.
    pub lifecycle_proof: String,
    pub origin: TemplateOrigin,
    pub requirements: TemplateRequirements,
    pub promotion: TemplatePromotion,
    pub data_storage: String,
    pub risk_notes: Vec<String>,
    /// One entry per setup field the import produces. Both directions are
    /// checked, so a field upstream adds cannot arrive unreviewed and a review
    /// left behind by a removed field cannot sit unnoticed.
    pub fields: BTreeMap<String, FieldReview>,
    /// The upstream definition, normalized to JSON exactly as the import
    /// report normalizes all of them. Kept verbatim so the mapping can be
    /// reproduced and audited without the archive.
    pub definition: String,
    #[serde(default)]
    pub config: Option<TemplateConfig>,
    /// Tags this review runs in place of the ones the definition names.
    #[serde(default)]
    pub image_pins: Vec<ImagePin>,
    /// Exact mount substitutions reviewed for this app; the upstream
    /// definition remains verbatim and any source drift refuses the install.
    #[serde(default)]
    pub storage_pins: Vec<StoragePin>,
    /// Per-service ceilings; omitted until a managed-engine proof can test them.
    #[serde(default)]
    pub resource_limits: Vec<ResourceLimit>,
    /// The files the definition's source copies into the app's data folder,
    /// carried verbatim like the definition itself so a review covers them.
    #[serde(default)]
    pub seeds: Vec<TemplateSeed>,
    /// Seconds the first start may take, for an app a review measured as
    /// slower than the default. Bounded so a broken app still fails in time.
    #[serde(default)]
    pub first_start_seconds: Option<u64>,
}

/// The longest first start a review may allow: fifteen minutes.
pub const MAX_FIRST_START_SECONDS: u64 = 900;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemplateSeed {
    /// Relative to the app's folder, inside `data/`.
    pub path: String,
    pub content: String,
}

impl ReviewedTemplate {
    /// Whether this template may be offered to anyone.
    ///
    /// Passing a review is not the same as being worth installing. A template
    /// can resolve cleanly, install, and still be one nobody should be handed
    /// — an image whose registry stopped rebuilding it years ago, say. This is
    /// the difference, and it is the project owner's decision to change.
    pub fn offerable(&self) -> bool {
        self.promotion.state == "approved"
    }

    /// The template this review permits, or why it does not permit one.
    pub fn plan_template(&self) -> Result<PlanTemplate, String> {
        let outcome = match self.origin.importer.as_str() {
            "caprover" => {
                if self.config.is_some() {
                    return Err("CapRover review cannot carry an unused companion config".into());
                }
                crate::importers::caprover::import(&self.id, &self.definition)
            }
            "runtipi" => {
                let config = self.config.as_ref().ok_or("Runtipi review requires its companion config")?;
                // Runtipi's store keeps an app at `apps/<id>/`; Local Store's
                // own definitions, written in the same format, keep it at
                // `definitions/apps/<id>/`. Either way the definition and its
                // config sit together and name this app.
                let definition = format!("apps/{}/docker-compose.json", self.id);
                let root = self.origin.path.strip_suffix(&definition);
                let expected = format!("{}apps/{}/config.json", root.unwrap_or_default(), self.id);
                if !matches!(root, Some("" | FIRST_PARTY_ROOT)) || config.path != expected {
                    return Err("Runtipi definition and config must name the reviewed app in the same pinned source".into());
                }
                let value: serde_json::Value = serde_json::from_str(&config.content)
                    .map_err(|_| "Runtipi companion config is invalid JSON")?;
                if !value.is_object() || value.get("id").and_then(serde_json::Value::as_str) != Some(&self.id) {
                    return Err("Runtipi companion config must identify the reviewed app".into());
                }
                crate::importers::runtipi::import(&self.id, &self.definition, Some(&config.content))
            }
            other => return Err(format!("no importer named {other:?} to map this definition")),
        }.map_err(|reason| format!("{}: {reason}", self.id))?;
        let mut template = outcome.template.ok_or_else(|| {
            let named: Vec<&str> = outcome
                .limitations
                .iter()
                .map(|limit| limit.feature())
                .collect();
            format!(
                "{}: the definition is no longer importable ({})",
                self.id,
                named.join(", ")
            )
        })?;

        // Applied before the audit below, so what gets audited is what runs.
        for pin in &self.image_pins {
            pin.check()
                .map_err(|reason| format!("{}: {reason}", self.id))?;
            let mut replaced = 0usize;
            for service in &mut template.plan.services {
                if service.image == pin.definition {
                    service.image = pin.replacement.clone();
                    replaced += 1;
                }
            }
            if replaced == 0 {
                return Err(format!(
                    "{}: image pin names {:?}, which this definition does not run",
                    self.id, pin.definition
                ));
            }
        }

        for pin in &self.storage_pins {
            if pin.reason.trim().len() <= 20 {
                return Err(format!("{}: storage pin gives no reason", self.id));
            }
            let service = template
                .plan
                .services
                .iter_mut()
                .find(|service| service.name == pin.service)
                .ok_or_else(|| {
                    format!(
                        "{}: storage pin names no service {:?}",
                        self.id, pin.service
                    )
                })?;
            let mut replaced = 0;
            for mount in &mut service.mounts {
                if matches!(mount, PlanMount::Directory { source, target, read_only: false }
                    if source == &pin.source && target == &pin.target)
                {
                    *mount = PlanMount::Volume {
                        name: pin.volume.clone(),
                        target: pin.target.clone(),
                        read_only: false,
                    };
                    replaced += 1;
                }
            }
            if replaced != 1 {
                return Err(format!(
                    "{}: storage pin for {:?} no longer matches one writable app-data mount",
                    self.id, pin.service
                ));
            }
            template.plan.named_volumes.push(pin.volume.clone());
        }

        for limit in &self.resource_limits {
            if limit.reason.trim().len() <= 20 {
                return Err(format!("{}: resource limit gives no reason", self.id));
            }
            let service = template
                .plan
                .services
                .iter_mut()
                .find(|service| service.name == limit.service)
                .ok_or_else(|| {
                    format!(
                        "{}: resource limit names no service {:?}",
                        self.id, limit.service
                    )
                })?;
            let overrides = &mut service.overrides;
            if overrides.memory_limit_bytes.is_some()
                || overrides.cpu_limit_millicores.is_some()
                || overrides.pids_limit.is_some()
            {
                return Err(format!(
                    "{}: resource limit for {:?} duplicates an existing ceiling",
                    self.id, limit.service
                ));
            }
            overrides.memory_limit_bytes = Some(limit.memory_bytes);
            overrides.cpu_limit_millicores = Some(limit.cpu_millicores);
            overrides.pids_limit = Some(limit.pids);
        }

        for field in &mut template.fields {
            let review = self.fields.get(&field.key).ok_or_else(|| {
                format!(
                    "{}: setup field {:?} has not been reviewed",
                    self.id, field.key
                )
            })?;
            field.label = review.label.clone();
            field.sensitive = review.sensitive;
        }
        for key in self.fields.keys() {
            if !template.fields.iter().any(|field| &field.key == key) {
                return Err(format!(
                    "{}: review names setup field {key:?}, which this definition no longer declares",
                    self.id
                ));
            }
        }
        // Every image that would run has to have been looked at, and every
        // image looked at has to still be one that runs.
        for service in &template.plan.services {
            if !self
                .requirements
                .images
                .iter()
                .any(|audit| audit.image == service.image)
            {
                return Err(format!(
                    "{}: image {:?} has not been audited",
                    self.id, service.image
                ));
            }
        }
        for audit in &self.requirements.images {
            if !template
                .plan
                .services
                .iter()
                .any(|service| service.image == audit.image)
            {
                return Err(format!(
                    "{}: audit names image {:?}, which this definition no longer runs",
                    self.id, audit.image
                ));
            }
        }

        if let Some(seconds) = self.first_start_seconds {
            let default = crate::runtime::FIRST_START_TIMEOUT.as_secs();
            if seconds <= default || seconds > MAX_FIRST_START_SECONDS {
                return Err(format!(
                    "{}: a first start of {seconds}s must be longer than the {default}s default and at most {MAX_FIRST_START_SECONDS}s",
                    self.id
                ));
            }
            template.first_start = Some(std::time::Duration::from_secs(seconds));
        }
        template.seeds = self
            .seeds
            .iter()
            .map(|seed| crate::setup::SeedFile {
                path: seed.path.clone(),
                content: seed.content.clone(),
            })
            .collect();

        // Install by digest, not by tag: the tag is what was audited, the
        // digest is what stops it meaning something else tomorrow.
        for service in &mut template.plan.services {
            service.digest = self
                .requirements
                .images
                .iter()
                .find(|audit| audit.image == service.image)
                .and_then(|audit| audit.index_digest.clone());
        }

        template
            .validate()
            .map_err(|reason| format!("{}: {reason}", self.id))?;
        Ok(template)
    }
}

const ACTIVEPIECES: &str = include_str!("activepieces.json");
const ACTUAL: &str = include_str!("actual.json");
const ADMINER: &str = include_str!("adminer.json");
const ANYTHINGLLM: &str = include_str!("anythingllm.json");
const BESZEL: &str = include_str!("beszel.json");
const BIG_AGI: &str = include_str!("big-agi.json");
const CHANGEDETECTION: &str = include_str!("changedetection.json");
const CODIMD: &str = include_str!("codimd.json");
const DIFY: &str = include_str!("dify.json");
const DOCMOST: &str = include_str!("docmost.json");
const FILESTASH: &str = include_str!("filestash.json");
const FLATNOTES: &str = include_str!("flatnotes.json");
const FLOWISE: &str = include_str!("flowise.json");
const GHOST_DEV: &str = include_str!("ghost-dev.json");
const GHOSTFOLIO: &str = include_str!("ghostfolio.json");
const GITEA: &str = include_str!("gitea.json");
const GLANCE: &str = include_str!("glance.json");
const GOTIFY: &str = include_str!("gotify.json");
const GRAFANA: &str = include_str!("grafana.json");
const GROCY: &str = include_str!("grocy.json");
const HOMER: &str = include_str!("homer.json");
const HUGINN: &str = include_str!("huginn.json");
const IMMICH: &str = include_str!("immich.json");
const JELLYFIN: &str = include_str!("jellyfin.json");
const JELLYSEERR: &str = include_str!("jellyseerr.json");
const JOPLIN: &str = include_str!("joplin.json");
const KANBOARD: &str = include_str!("kanboard.json");
const KHOJ: &str = include_str!("khoj.json");
const KOTAEMON: &str = include_str!("kotaemon.json");
const LANGFLOW: &str = include_str!("langflow.json");
const LIBRECHAT: &str = include_str!("librechat.json");
const LOBEHUB: &str = include_str!("lobehub.json");
const MAXUN: &str = include_str!("maxun.json");
const METABASE: &str = include_str!("metabase.json");
const MONICA: &str = include_str!("monica.json");
const NAVIDROME: &str = include_str!("navidrome.json");
const NODERED: &str = include_str!("nodered.json");
const NTFY: &str = include_str!("ntfy.json");
const OMBI: &str = include_str!("ombi.json");
const OPEN_WEBUI: &str = include_str!("open-webui.json");
const PAIRDROP: &str = include_str!("pairdrop.json");
const PAPERCLIP: &str = include_str!("paperclip.json");
const PENPOT: &str = include_str!("penpot.json");
const PRIVATEBIN: &str = include_str!("privatebin.json");
const SILLYTAVERN: &str = include_str!("sillytavern.json");
const SIM: &str = include_str!("sim.json");
const TANDOOR: &str = include_str!("tandoor.json");
const TAUTULLI: &str = include_str!("tautulli.json");
const TRILIUM: &str = include_str!("trilium.json");
const UMAMI_ANALYTICS: &str = include_str!("umami-analytics.json");
const VANE: &str = include_str!("vane.json");
const VAULTWARDEN: &str = include_str!("vaultwarden.json");
const VIKUNJA: &str = include_str!("vikunja.json");
const WALLOS: &str = include_str!("wallos.json");
const WEKAN: &str = include_str!("wekan.json");
const WHOOGLE: &str = include_str!("whoogle.json");
const WORDPRESS: &str = include_str!("wordpress.json");

/// Where Local Store keeps definitions it wrote itself, in Runtipi's format.
///
/// For an app no store packages well — Paperclip is in none, and Runtipi's
/// Penpot runs `:latest` with an exporter pointed at the wrong port. A
/// definition here is reviewed exactly like an imported one; the difference
/// is only who is answerable for it.
pub const FIRST_PARTY_ROOT: &str = "definitions/";

/// Every template a review has passed. Being here is not being offered.
pub fn reviewed_templates() -> Vec<ReviewedTemplate> {
    [
        ACTIVEPIECES,
        ACTUAL,
        ADMINER,
        ANYTHINGLLM,
        BESZEL,
        BIG_AGI,
        CHANGEDETECTION,
        CODIMD,
        DIFY,
        DOCMOST,
        FILESTASH,
        FLATNOTES,
        FLOWISE,
        GHOST_DEV,
        GHOSTFOLIO,
        GITEA,
        GLANCE,
        GOTIFY,
        GRAFANA,
        GROCY,
        HOMER,
        HUGINN,
        IMMICH,
        JELLYFIN,
        JELLYSEERR,
        JOPLIN,
        KANBOARD,
        KHOJ,
        KOTAEMON,
        LANGFLOW,
        LIBRECHAT,
        LOBEHUB,
        MAXUN,
        METABASE,
        MONICA,
        NAVIDROME,
        NODERED,
        NTFY,
        OMBI,
        OPEN_WEBUI,
        PAIRDROP,
        PAPERCLIP,
        PENPOT,
        PRIVATEBIN,
        SILLYTAVERN,
        SIM,
        TANDOOR,
        TAUTULLI,
        TRILIUM,
        UMAMI_ANALYTICS,
        VANE,
        VAULTWARDEN,
        VIKUNJA,
        WALLOS,
        WEKAN,
        WHOOGLE,
        WORDPRESS,
    ]
    .into_iter()
    .map(|source| serde_json::from_str(source).expect("bundled reviewed templates must parse"))
    .collect()
}

pub fn reviewed_template(id: &str) -> Option<ReviewedTemplate> {
    reviewed_templates()
        .into_iter()
        .find(|template| template.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtipi_review() -> ReviewedTemplate {
        reviewed_template("privatebin").expect("privatebin is reviewed")
    }

    #[test]
    fn runtipi_review_uses_the_existing_importer_and_requires_companion_identity() {
        let reviewed = runtipi_review();
        let plan = reviewed.plan_template().unwrap();
        assert_eq!(plan.plan.services.len(), 1);
        assert!(plan.fields.is_empty());
        // One service, published, and nothing left for a person to answer.
        assert!(plan.plan.published().is_some());
        assert!(plan.secrets.is_empty());
        let mut missing = reviewed.clone();
        missing.config = None;
        assert!(missing.plan_template().unwrap_err().contains("requires"));
        let mut wrong = reviewed.clone();
        wrong.config.as_mut().unwrap().path = "apps/other/config.json".into();
        assert!(wrong.plan_template().is_err());
        let mut wrong = reviewed.clone();
        wrong.config.as_mut().unwrap().content = r#"{"id":"other"}"#.into();
        assert!(wrong.plan_template().is_err());
        let mut wrong = reviewed;
        wrong.definition = r#"{"services":[{"name":"privatebin","image":"privatebin/nginx-fpm-alpine:2.0.6","isMain":true,"internalPort":8080,"privileged":true}]}"#.into();
        assert!(wrong.plan_template().is_err());
    }

    #[test]
    fn runtipi_review_rejects_unreviewed_images_and_fields() {
        let mut reviewed = runtipi_review();
        reviewed.requirements.images.clear();
        assert!(reviewed.plan_template().unwrap_err().contains("audited"));
        let mut reviewed = runtipi_review();
        reviewed.config.as_mut().unwrap().content = r#"{"id":"privatebin","form_fields":[{"type":"text","env_variable":"TZ","label":"Zone","default":"UTC"}]}"#.into();
        // A newly declared setup field must not silently inherit an old review.
        reviewed.definition = reviewed.definition.replace(
            "\"internalPort\": 8080",
            "\"environment\": [{\"key\":\"TZ\",\"value\":\"${TZ}\"}], \"internalPort\": 8080",
        );
        assert!(reviewed.plan_template().is_err());
    }

    #[test]
    fn every_reviewed_template_resolves_to_a_valid_plan() {
        let templates = reviewed_templates();
        assert!(!templates.is_empty(), "the allowlist is empty");
        for reviewed in templates {
            let template = reviewed
                .plan_template()
                .unwrap_or_else(|reason| panic!("{reason}"));
            assert!(
                !template.plan.services.is_empty(),
                "{} deploys nothing",
                reviewed.id
            );
            // A reviewed template has to render, not merely construct.
            let compose = template.plan.to_compose().expect("plan should render");
            assert!(!compose.contains("$$cap_"), "{}: {compose}", reviewed.id);

            // Everything a recipe must prove about itself.
            assert_eq!(reviewed.schema_version, 1);
            assert!(!reviewed.risk_notes.is_empty(), "{}", reviewed.id);
            // A named proof that is not there is worse than none: it reads
            // as verification nobody can check.
            let proof =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&reviewed.lifecycle_proof);
            assert!(
                reviewed.lifecycle_proof.starts_with("docs/evidence/") && proof.is_file(),
                "{} names a lifecycle proof that is not there: {}",
                reviewed.id,
                reviewed.lifecycle_proof
            );
            assert!(
                reviewed.origin.revision.len() == 40
                    && reviewed
                        .origin
                        .revision
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit()),
                "{} pins no upstream commit",
                reviewed.id
            );
            assert!(
                !reviewed.requirements.images.is_empty(),
                "{} audits no image",
                reviewed.id
            );
            for audit in &reviewed.requirements.images {
                assert!(
                    !audit.container_platforms.is_empty(),
                    "{}: {} claims no platform",
                    reviewed.id,
                    audit.image
                );
                assert_eq!(
                    audit.digests.keys().cloned().collect::<Vec<_>>(),
                    audit.container_platforms,
                    "{}: {} pins a different set of platforms than it claims",
                    reviewed.id,
                    audit.image
                );
                for digest in audit.digests.values() {
                    assert!(digest.starts_with("sha256:"), "{}", reviewed.id);
                }
            }
            assert!(
                matches!(reviewed.promotion.state.as_str(), "approved" | "withheld"),
                "{} records no promotion decision",
                reviewed.id
            );
            assert!(
                reviewed.promotion.reason.len() > 20,
                "{} gives no reason for its promotion decision",
                reviewed.id
            );
            for service in &template.plan.services {
                let tag = service.image.rsplit_once(':').map(|(_, tag)| tag);
                assert!(
                    tag.is_some_and(|tag| !matches!(tag, "latest" | "stable" | "main")),
                    "{} runs an unpinned image",
                    reviewed.id
                );
            }
        }
    }

    #[test]
    fn a_review_decides_which_answers_are_credentials() {
        let reviewed = reviewed_template("codimd").expect("codimd is reviewed");
        let template = reviewed.plan_template().unwrap();
        let review = template.setup_review().unwrap();

        // The importer marks every field sensitive because upstream cannot
        // say. Without a review this would be a masked box with its default
        // withheld, which is the wrong way to ask for a time zone.
        let zone = review
            .fields
            .iter()
            .find(|field| field.key == "CAP_TIMEZONE")
            .expect("codimd asks for a time zone");
        assert_eq!(zone.control, "text");
        assert!(!zone.sensitive);
        assert_eq!(zone.default.as_deref(), Some("Europe/London"));
        assert_eq!(zone.label, "Time zone");

        // The generated credential is counted, never asked for or shown.
        assert_eq!(review.generated_credential_count, 1);
        assert!(!review.fields.iter().any(|field| field.key.contains("PASS")));
    }

    /// Every app this project offers to install, named here on purpose.
    ///
    /// This list started empty, which was the honest state of the project
    /// before anything had been qualified. Emptiness was never the point,
    /// though — the point is that approving an app is a decision somebody
    /// makes, so it has to appear in a diff rather than arrive as a side
    /// effect of a manifest edit or a passing test. Adding a second name here
    /// costs exactly as much deliberation as the first one did.
    const APPROVED: &[&str] = &[
        "adminer",
        "anythingllm",
        "beszel",
        "big-agi",
        "changedetection",
        "dify",
        "docmost",
        "flatnotes",
        "flowise",
        "ghostfolio",
        "gitea",
        "glance",
        "grafana",
        "grocy",
        "homer",
        "huginn",
        "immich",
        "jellyfin",
        "joplin",
        "kanboard",
        "khoj",
        "kotaemon",
        "langflow",
        "librechat",
        "lobehub",
        "maxun",
        "metabase",
        "monica",
        "navidrome",
        "nodered",
        "ntfy",
        "open-webui",
        "pairdrop",
        "paperclip",
        "penpot",
        "privatebin",
        "sillytavern",
        "sim",
        "tandoor",
        "tautulli",
        "trilium",
        "umami-analytics",
        "vane",
        "vaultwarden",
        "vikunja",
        "wallos",
        "wekan",
        "whoogle",
        "wordpress",
    ];

    #[test]
    fn no_reviewed_template_is_offerable_without_an_explicit_approval() {
        let mut offerable: Vec<String> = reviewed_templates()
            .into_iter()
            .filter(ReviewedTemplate::offerable)
            .map(|template| template.id)
            .collect();
        // Compared as a set, so reordering the allowlist is not a failure but
        // adding to it still is.
        offerable.sort();
        assert_eq!(
            offerable, APPROVED,
            "the approved templates changed without this list changing with them"
        );
    }

    /// An approval is a claim that somebody checked this app, so the evidence
    /// it rests on has to be there. A withheld template is held to the same
    /// standard everywhere else; this is the part that only matters once an
    /// app can actually reach a person.
    /// A definition Local Store wrote has no upstream to compare against, so
    /// the file in this repository is the source of truth, and the manifest
    /// has to carry it byte for byte — not an edited copy.
    #[test]
    fn a_first_party_definition_is_the_file_in_the_tree() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for reviewed in reviewed_templates() {
            if !reviewed.origin.path.starts_with(FIRST_PARTY_ROOT) {
                continue;
            }
            let on_disk =
                std::fs::read_to_string(root.join(&reviewed.origin.path)).unwrap_or_else(|_| {
                    panic!(
                        "{}: {} is not in the tree",
                        reviewed.id, reviewed.origin.path
                    )
                });
            assert_eq!(
                on_disk.replace("\r\n", "\n"),
                reviewed.definition.replace("\r\n", "\n"),
                "{}: the manifest carries a different definition than {}",
                reviewed.id,
                reviewed.origin.path
            );
            let folder = root.join(&reviewed.origin.path);
            let (on_disk, binary) = crate::importers::runtipi::read_seeds(folder.parent().unwrap());
            assert!(
                binary.is_empty(),
                "{}: binary seeds {binary:?}",
                reviewed.id
            );
            let carried: Vec<(String, String)> = reviewed
                .seeds
                .iter()
                .map(|seed| (seed.path.clone(), seed.content.replace("\r\n", "\n")))
                .collect();
            let expected: Vec<(String, String)> = on_disk
                .into_iter()
                .map(|seed| (seed.path, seed.content.replace("\r\n", "\n")))
                .collect();
            assert_eq!(
                carried, expected,
                "{}: seed files differ from the tree",
                reviewed.id
            );
            let config = reviewed
                .config
                .as_ref()
                .expect("a first-party definition has its config");
            let on_disk = std::fs::read_to_string(root.join(&config.path))
                .unwrap_or_else(|_| panic!("{}: {} is not in the tree", reviewed.id, config.path));
            assert_eq!(
                on_disk.replace("\r\n", "\n"),
                config.content.replace("\r\n", "\n"),
                "{}: the manifest carries a different config than {}",
                reviewed.id,
                config.path
            );
        }
    }

    /// A review may give a slow app longer to start, but not forever and
    /// not less than everyone already gets.
    #[test]
    fn a_first_start_allowance_is_bounded_and_reaches_the_plan() {
        let mut reviewed = reviewed_template("khoj").expect("khoj is reviewed");
        for refused in [
            60,
            crate::runtime::FIRST_START_TIMEOUT.as_secs(),
            MAX_FIRST_START_SECONDS + 1,
        ] {
            reviewed.first_start_seconds = Some(refused);
            assert!(reviewed.plan_template().is_err(), "{refused}s was accepted");
        }
        reviewed.first_start_seconds = Some(600);
        let template = reviewed.plan_template().expect("600s is within bounds");
        assert_eq!(
            template.first_start,
            Some(std::time::Duration::from_secs(600))
        );
    }

    #[test]
    fn an_approved_template_carries_the_evidence_its_approval_claims() {
        for id in APPROVED {
            let reviewed = reviewed_template(id).expect("an approved template must exist");
            assert!(reviewed.offerable(), "{id} is listed but not approved");
            assert!(
                reviewed.promotion.reason.len() > 20,
                "{id} gives no reason for its approval"
            );
            assert!(
                !reviewed.verified_at.is_empty(),
                "{id} records no review date"
            );
            let proof =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&reviewed.lifecycle_proof);
            assert!(
                proof.is_file(),
                "{id} names a lifecycle proof that is not there"
            );
            // It has to map, and to map to something installable.
            let template = reviewed
                .plan_template()
                .expect("an approved template must map");
            assert!(
                template.plan.published().is_some(),
                "{id} publishes no address to open"
            );
            // And the proof has to be about what runs. An image pin changes
            // the images without changing the file the review points at, so
            // a proof of the old image would otherwise keep vouching for the
            // new one.
            let evidence: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&proof).unwrap())
                    .unwrap_or_else(|_| panic!("{id}'s proof is not JSON"));
            let mut proven: Vec<String> = evidence["images"]
                .as_array()
                .unwrap_or_else(|| panic!("{id}'s proof records no images"))
                .iter()
                .filter_map(|image| image.as_str().map(str::to_owned))
                .collect();
            let mut runs: Vec<String> = template
                .plan
                .services
                .iter()
                .map(|service| service.image.clone())
                .collect();
            proven.sort();
            runs.sort();
            assert_eq!(
                proven, runs,
                "{id}'s proof is about different images than it runs; qualify it as offered"
            );
            assert_eq!(
                evidence["passed"].as_bool(),
                Some(true),
                "{id} is approved on a proof that did not pass"
            );
            // What installs is pinned by digest, for every image it runs.
            for service in &template.plan.services {
                assert!(
                    service.digest.is_some(),
                    "{id}: {} would install by tag; record its audited index digest",
                    service.image
                );
            }
            // A check that passes trivially must not be quoted as though it
            // had tested something. Four approvals said their generated
            // credentials survived a reinstall for apps that generate none.
            let generates_none = evidence["steps"].as_array().is_some_and(|steps| {
                steps.iter().any(|step| {
                    step["step"]
                        .as_str()
                        .is_some_and(|name| name.starts_with("generates no credentials"))
                })
            });
            assert!(
                !(generates_none && reviewed.promotion.reason.contains("credentials intact")),
                "{id}'s approval claims credentials survived, but its proof says it generates none"
            );
        }
    }

    /// The other direction of the same decision CodiMD's time zone shows.
    ///
    /// Runtipi marks a field sensitive only when upstream types it `password`.
    /// flatnotes types its password as plain `text`, so without a review the
    /// setup form would render the one field on that form that is genuinely a
    /// credential as a visible box, with its value echoed back on screen. A
    /// review is where somebody who read the app says otherwise.
    #[test]
    fn a_review_masks_a_credential_upstream_declared_as_plain_text() {
        let reviewed = reviewed_template("flatnotes").expect("flatnotes is reviewed");
        // Upstream really does declare it as text; if that ever changes this
        // test stops being about anything and should be revisited.
        assert!(reviewed
            .config
            .as_ref()
            .expect("flatnotes carries its config")
            .content
            .contains(
                "\"type\": \"text\",
      \"label\": \"Flatnotes Password\""
            ));

        let review = reviewed
            .plan_template()
            .expect("flatnotes should map")
            .setup_review()
            .expect("flatnotes should project");

        let password = review
            .fields
            .iter()
            .find(|field| field.key == "FLATNOTES_PASSWORD")
            .expect("flatnotes asks for a password");
        assert_eq!(password.control, "password");
        assert!(password.sensitive);
        assert_eq!(password.label, "Password");
        // A sensitive field's default is described, never sent.
        assert_eq!(password.default, None);

        // The two answers that are not credentials stay ordinary text, so a
        // username is not asked for behind dots.
        for key in ["FLATNOTES_USERNAME", "FLATNOTES_AUTH_TYPE"] {
            let field = review
                .fields
                .iter()
                .find(|field| field.key == key)
                .unwrap_or_else(|| panic!("flatnotes asks for {key}"));
            assert_eq!(field.control, "text", "{key}");
            assert!(!field.sensitive, "{key}");
        }

        // Both generated credentials are counted and neither is ever asked
        // for or shown.
        assert_eq!(review.generated_credential_count, 2);
        assert!(!review
            .fields
            .iter()
            .any(|field| field.key.contains("SECRET") || field.key.contains("TOTP")));
    }

    /// The override exists so an app is not stuck on whatever tag an upstream
    /// catalogue last bumped. It must stay an override of a *tag*, though —
    /// everything below is a way it could become a second definition wearing
    /// the first one's provenance.
    #[test]
    fn an_image_pin_may_move_a_tag_and_nothing_else() {
        let reviewed = reviewed_template("nodered").expect("nodered is reviewed");
        // The definition says 5.0.6; what runs is what the review pinned.
        assert!(reviewed.definition.contains("nodered/node-red:5.0.6"));
        let template = reviewed.plan_template().expect("nodered should map");
        assert_eq!(template.plan.services[0].image, "nodered/node-red:5.0.7");

        let refuse = |pin: ImagePin, expect: &str| {
            let mut broken = reviewed.clone();
            broken.image_pins = vec![pin];
            let error = broken
                .plan_template()
                .expect_err("this pin should have been refused");
            assert!(error.contains(expect), "{error}");
        };
        // A different image is not a pin.
        refuse(
            ImagePin {
                definition: "nodered/node-red:5.0.6".into(),
                replacement: "someone-else/node-red:5.0.7".into(),
                reason: "a reason long enough to pass the length check".into(),
            },
            "not the image",
        );
        // A pin for something this definition does not run is a stale review.
        refuse(
            ImagePin {
                definition: "nodered/node-red:4.0.0".into(),
                replacement: "nodered/node-red:5.0.7".into(),
                reason: "a reason long enough to pass the length check".into(),
            },
            "does not run",
        );
        // A decision with no stated reason is not a review.
        refuse(
            ImagePin {
                definition: "nodered/node-red:5.0.6".into(),
                replacement: "nodered/node-red:5.0.7".into(),
                reason: "because".into(),
            },
            "no reason",
        );
        // An untagged replacement would float, which is what pinning prevents.
        refuse(
            ImagePin {
                definition: "nodered/node-red:5.0.6".into(),
                replacement: "nodered/node-red".into(),
                reason: "a reason long enough to pass the length check".into(),
            },
            "names no tag",
        );
    }

    #[test]
    fn storage_pins_replace_only_the_reviewed_gitea_mounts() {
        let reviewed = reviewed_template("gitea").expect("gitea is reviewed");
        let template = reviewed.plan_template().expect("gitea should map");
        assert_eq!(
            template.plan.named_volumes,
            vec!["gitea-data", "gitea-postgres"]
        );
        let app = template
            .plan
            .services
            .iter()
            .find(|service| service.name == "gitea")
            .expect("app service exists");
        assert!(
            matches!(&app.mounts[0], PlanMount::Volume { name, target, read_only: false }
            if name == "gitea-data" && target == "/data")
        );
        let db = template
            .plan
            .services
            .iter()
            .find(|service| service.name == "gitea-db")
            .expect("database service exists");
        assert!(
            matches!(&db.mounts[0], PlanMount::Volume { name, target, read_only: false }
            if name == "gitea-postgres" && target == "/var/lib/postgresql/data")
        );

        let mut stale = reviewed.clone();
        stale.storage_pins[0].source = "data/elsewhere".into();
        assert!(stale
            .plan_template()
            .unwrap_err()
            .contains("no longer matches"));
        let mut duplicate = reviewed;
        duplicate
            .storage_pins
            .push(duplicate.storage_pins[0].clone());
        assert!(duplicate
            .plan_template()
            .unwrap_err()
            .contains("no longer matches"));
    }

    #[test]
    fn resource_limits_bind_to_one_reviewed_service_and_refuse_drift() {
        let reviewed = reviewed_template("flatnotes").expect("flatnotes is reviewed");
        let template = reviewed.plan_template().expect("flatnotes should map");
        let limits = &template.plan.services[0].overrides;
        assert_eq!(limits.memory_limit_bytes, Some(536_870_912));
        assert_eq!(limits.cpu_limit_millicores, Some(2000));
        assert_eq!(limits.pids_limit, Some(512));

        let mut stale = reviewed.clone();
        stale.resource_limits[0].service = "removed-service".into();
        assert!(stale
            .plan_template()
            .unwrap_err()
            .contains("names no service"));
        let mut duplicate = reviewed.clone();
        duplicate
            .resource_limits
            .push(reviewed.resource_limits[0].clone());
        assert!(duplicate
            .plan_template()
            .unwrap_err()
            .contains("duplicates an existing ceiling"));
        let mut invalid = reviewed;
        invalid.resource_limits[0].memory_bytes = 0;
        assert!(invalid.plan_template().is_err());
    }

    /// The audit has to be about what runs, not about what the definition said
    /// before the review moved it.
    #[test]
    fn an_image_pin_must_be_audited_at_the_tag_it_moves_to() {
        let mut reviewed = reviewed_template("nodered").expect("nodered is reviewed");
        assert_eq!(
            reviewed.requirements.images[0].image,
            "nodered/node-red:5.0.7"
        );
        // Auditing the tag the definition names, rather than the one that
        // runs, is exactly the mistake this must not permit.
        reviewed.requirements.images[0].image = "nodered/node-red:5.0.6".into();
        let error = reviewed
            .plan_template()
            .expect_err("the audit must not drift");
        assert!(error.contains("has not been audited"), "{error}");
    }

    #[test]
    fn an_image_nobody_audited_stops_the_template() {
        let mut reviewed = reviewed_template("codimd").unwrap();
        reviewed.requirements.images.remove(0);
        let error = reviewed.plan_template().unwrap_err();
        assert!(error.contains("has not been audited"), "{error}");
    }

    #[test]
    fn a_field_nobody_reviewed_stops_the_template_rather_than_shipping_masked() {
        let mut reviewed = reviewed_template("codimd").unwrap();
        reviewed.fields.remove("CAP_TIMEZONE");
        let error = reviewed.plan_template().unwrap_err();
        assert!(error.contains("has not been reviewed"), "{error}");

        // And a review left behind by a field upstream removed is caught too,
        // because it means the review was written against a different app.
        let mut stale = reviewed_template("codimd").unwrap();
        stale.fields.insert(
            "CAP_GONE".into(),
            FieldReview {
                label: "Gone".into(),
                sensitive: false,
            },
        );
        let error = stale.plan_template().unwrap_err();
        assert!(error.contains("no longer declares"), "{error}");
    }
}
