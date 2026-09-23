//! A normalized multi-service deployment plan.
//!
//! Reviewed recipes currently carry a literal Compose file, which works for one
//! service and stops working the moment an app needs a database beside it. A
//! plan describes *what* to deploy — services, images, storage, one published
//! endpoint — and renders the Compose file, so the safety rules are checked
//! against structure rather than re-read out of hand-written YAML.
//!
//! Nothing ships from a plan yet. The gate for this model is that it can
//! express every recipe already shipping and render each one byte-for-byte;
//! `plan_for_recipe` builds those plans and a test asserts the equality.
use crate::recipes::Recipe;
mod healthcheck;
pub use healthcheck::{HealthTest, PlanHealthcheck};

/// A service's storage. Bind mounts live inside the app's own project
/// directory; named volumes are managed by Docker.
///
/// `read_only` is a narrowing, never a widening: it can only take away a
/// container's ability to write to a path it would otherwise own. Definitions
/// that ask for it are usually mounting configuration a container should read
/// and not rewrite, so honouring it costs nothing and refusing it would have
/// meant importing the app with *more* access than it asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanMount {
    /// `./<source>:<target>`, relative to the managed project directory.
    Directory {
        source: String,
        target: String,
        read_only: bool,
    },
    /// `<name>:<target>`, where `name` is a declared named volume.
    Volume {
        name: String,
        target: String,
        read_only: bool,
    },
    /// A folder on this computer that a person chose, mounted at `target`.
    ///
    /// This is the only mount that reaches outside the storage this product
    /// manages, and it exists because the apps people most want — a photo
    /// library, a music collection — keep their files where the person keeps
    /// them. `source` holds a `${KEY}` placeholder until an answer replaces
    /// it, and `crate::folders::share_folder` decides whether that answer is
    /// allowed. A plan may not carry a literal path here before resolution: it
    /// would be a mount nobody consented to.
    Host {
        source: String,
        target: String,
        read_only: bool,
    },
}
impl PlanMount {
    /// A writable bind mount, the shape every reviewed recipe uses.
    pub fn directory(source: &str, target: &str) -> Self {
        Self::Directory {
            source: source.to_owned(),
            target: target.to_owned(),
            read_only: false,
        }
    }

    fn rendered(&self) -> String {
        // `:ro` is appended only when asked for, so a writable mount renders
        // exactly the text the reviewed recipes already ship.
        let suffix = if self.read_only() { ":ro" } else { "" };
        match self {
            Self::Directory { source, target, .. } => format!("./{source}:{target}{suffix}"),
            Self::Volume { name, target, .. } => format!("{name}:{target}{suffix}"),
            Self::Host { source, target, .. } => format!("{source}:{target}{suffix}"),
        }
    }
    fn target(&self) -> &str {
        match self {
            Self::Directory { target, .. }
            | Self::Volume { target, .. }
            | Self::Host { target, .. } => target,
        }
    }
    fn read_only(&self) -> bool {
        match self {
            Self::Directory { read_only, .. }
            | Self::Volume { read_only, .. }
            | Self::Host { read_only, .. } => *read_only,
        }
    }
}

/// Arguments Compose accepts in either of two forms that do not mean the same
/// thing: a bare string is split the way a shell would split it, while a list
/// is handed to the container unsplit. Keeping them apart is what stops an
/// import quietly changing what a container runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanArgs {
    /// `command: serve --fast`
    Shell(String),
    /// `command: ["serve", "--fast"]`
    Exec(Vec<String>),
}

impl PlanArgs {
    fn validate(&self, field: &str) -> Result<(), String> {
        let parts: Vec<&str> = match self {
            Self::Shell(line) => vec![line.as_str()],
            Self::Exec(parts) => parts.iter().map(String::as_str).collect(),
        };
        if parts.is_empty() || parts.iter().all(|part| part.trim().is_empty()) {
            return Err(format!("{field} is empty"));
        }
        if parts.len() > 64 {
            return Err(format!("{field} has more arguments than a plan will carry"));
        }
        for part in &parts {
            if part.len() > 4096 {
                return Err(format!("{field} argument is too long"));
            }
            if part.chars().any(char::is_control) {
                return Err(format!("{field} contains control characters"));
            }
        }
        Ok(())
    }

    fn render(&self, field: &str, out: &mut String) {
        match self {
            Self::Shell(line) => out.push_str(&format!("    {field}: {}\n", scalar(line))),
            Self::Exec(parts) => {
                out.push_str(&format!("    {field}:\n"));
                for part in parts {
                    out.push_str(&format!("      - {}\n", scalar(part)));
                }
            }
        }
    }
}

/// Startup settings a definition may override on top of what its image
/// declares. Grouped so a service gains one field rather than three, and so a
/// service that overrides nothing says so in one place.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanOverrides {
    /// A subset of depends_on that must pass a container health check first.
    pub healthy_dependencies: std::collections::BTreeSet<String>,
    /// A subset of depends_on that must run to completion first: a one-shot
    /// job, such as the database migration Sim runs before its app starts.
    /// Naming a service here is what makes it a job — it runs once, is not
    /// restarted, and its exit status decides whether the app starts.
    pub completed_dependencies: std::collections::BTreeSet<String>,
    pub healthcheck: Option<PlanHealthcheck>,
    /// Replaces the command the image would run.
    pub command: Option<PlanArgs>,
    /// Replaces the entry point the image declares.
    pub entrypoint: Option<PlanArgs>,
    /// A name the container answers to on the project network.
    pub hostname: Option<String>,
    /// The signal Docker sends to stop the container. Postgres needs SIGINT
    /// for a fast, clean shutdown; the default SIGTERM makes it wait for every
    /// client to leave, and Docker kills it before it does.
    pub stop_signal: Option<String>,
    /// Explicit container ceilings, set after qualification has measured the
    /// app on the managed engine. None preserves the reviewed definition.
    pub memory_limit_bytes: Option<u64>,
    pub cpu_limit_millicores: Option<u32>,
    pub pids_limit: Option<u32>,
}

impl PlanOverrides {
    fn validate(&self) -> Result<(), String> {
        if self
            .memory_limit_bytes
            .is_some_and(|bytes| bytes < 6 * 1024 * 1024)
        {
            return Err("memory limit must be at least 6 MiB".into());
        }
        if self.cpu_limit_millicores == Some(0) {
            return Err("CPU limit must be greater than zero".into());
        }
        if self.pids_limit == Some(0) {
            return Err("PID limit must be greater than zero".into());
        }
        if let Some(check) = &self.healthcheck {
            check.validate()?;
        }
        if let Some(command) = &self.command {
            command.validate("command")?;
        }
        if let Some(entrypoint) = &self.entrypoint {
            entrypoint.validate("entrypoint")?;
        }
        if let Some(signal) = &self.stop_signal {
            // A name like SIGINT, or a number — nothing that could carry
            // anything else into the line it renders into.
            let named = signal.strip_prefix("SIG").is_some_and(|rest| {
                !rest.is_empty()
                    && rest
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            });
            let numbered = !signal.is_empty()
                && signal.len() <= 2
                && signal.chars().all(|c| c.is_ascii_digit());
            if !named && !numbered {
                return Err(format!("stop signal {signal:?} is not a signal name"));
            }
        }
        if let Some(hostname) = &self.hostname {
            // Docker rejects anything else, and a plan should say so first.
            if hostname.is_empty()
                || hostname.len() > 63
                || !hostname
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                || hostname.starts_with('-')
                || hostname.ends_with('-')
            {
                return Err(format!("hostname {hostname:?} is not a valid host name"));
            }
        }
        Ok(())
    }

    fn render(&self, out: &mut String) {
        if let Some(bytes) = self.memory_limit_bytes {
            out.push_str(&format!("    mem_limit: {bytes}\n"));
        }
        if let Some(millicores) = self.cpu_limit_millicores {
            out.push_str(&format!(
                "    cpus: \"{}.{:03}\"\n",
                millicores / 1000,
                millicores % 1000
            ));
        }
        if let Some(pids) = self.pids_limit {
            out.push_str(&format!("    pids_limit: {pids}\n"));
        }
        if let Some(check) = &self.healthcheck {
            check.render(out);
        }
        if let Some(hostname) = &self.hostname {
            out.push_str(&format!("    hostname: {}\n", scalar(hostname)));
        }
        if let Some(signal) = &self.stop_signal {
            out.push_str(&format!("    stop_signal: {signal}\n"));
        }
        // Entry point before command, the order Docker applies them in.
        if let Some(entrypoint) = &self.entrypoint {
            entrypoint.render("entrypoint", out);
        }
        if let Some(command) = &self.command {
            command.render("command", out);
        }
    }
}

/// The one endpoint a plan exposes on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishedPort {
    pub host: u16,
    pub container: u16,
}

/// How many second addresses one plan may publish beside its main one.
///
/// Some apps' own pages call a second server directly: Sim's browser keeps
/// a socket open to its realtime service, Maxun's frontend calls its backend,
/// and LobeHub uploads files straight to its object store. Each needs an
/// address on this computer. A companion is published on loopback like the
/// main port, but it is never opened or probed — the main address is still
/// the app.
pub const MAX_COMPANION_PORTS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanService {
    pub name: String,
    pub image: String,
    /// The audited manifest-list digest for `image`, when a review recorded
    /// one. It renders as `image@digest`, so what runs is what was audited
    /// even if somebody pushes the tag again. A candidate being qualified has
    /// none; everything offered has one.
    pub digest: Option<String>,
    pub environment: Vec<(String, String)>,
    /// A second address the app's own pages reach this service on. The
    /// service learns it through `${LOCAL_STORE_URL_<SERVICE>}` and
    /// `${LOCAL_STORE_PORT_<SERVICE>}`, filled once the port is settled.
    pub companion: Option<PublishedPort>,
    /// The networks this service joins, when it is not simply the project's
    /// own. `default` names that one. Dify keeps its code sandbox on a network
    /// with no route out except through its proxy, which is the point of it.
    pub networks: Vec<String>,
    pub published: Option<PublishedPort>,
    pub mounts: Vec<PlanMount>,
    pub depends_on: Vec<String>,
    /// Startup settings that replace what the image declares. Default means
    /// the image is left to start itself the way its author intended.
    pub overrides: PlanOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPlan {
    pub id: String,
    pub services: Vec<PlanService>,
    pub named_volumes: Vec<String>,
    /// Networks with no route out of Docker. A service reaches the internet
    /// from one only through something that also sits on the default network.
    pub internal_networks: Vec<String>,
}

impl DeploymentPlan {
    /// Compose names the container after the plan, and after the service too
    /// once there is more than one, so two apps never collide.
    fn container_name(&self, service: &PlanService) -> String {
        if service.name == self.id {
            format!("local-store-{}", self.id)
        } else {
            format!("local-store-{}-{}", self.id, service.name)
        }
    }

    /// The single service that publishes a port, which is what the launcher
    /// opens and probes.
    pub fn published(&self) -> Option<(&PlanService, PublishedPort)> {
        self.services
            .iter()
            .find_map(|service| service.published.map(|port| (service, port)))
    }

    /// Move the one published endpoint onto a different host port.
    ///
    /// The container port never moves: that is the port the application
    /// actually listens on. Only the address this computer answers on changes.
    pub fn set_published_host(&mut self, host: u16) {
        if let Some(port) = self
            .services
            .iter_mut()
            .find_map(|service| service.published.as_mut())
        {
            port.host = host;
        }
    }

    /// Every second address, by the service that answers on it.
    pub fn companions(&self) -> Vec<(&PlanService, PublishedPort)> {
        self.services
            .iter()
            .filter_map(|service| service.companion.map(|port| (service, port)))
            .collect()
    }

    /// Move one service's second address onto a different host port.
    pub fn set_companion_host(&mut self, service: &str, host: u16) {
        if let Some(port) = self
            .services
            .iter_mut()
            .find(|candidate| candidate.name == service)
            .and_then(|candidate| candidate.companion.as_mut())
        {
            port.host = host;
        }
    }

    /// Whether this service is a one-shot job: something waits for it to
    /// finish rather than to start.
    pub fn is_job(&self, name: &str) -> bool {
        self.services
            .iter()
            .any(|service| service.overrides.completed_dependencies.contains(name))
    }

    /// Bind-mounted directories, which the installer creates before starting.
    pub fn data_directories(&self) -> Vec<&str> {
        let mut directories: Vec<&str> = self
            .services
            .iter()
            .flat_map(|service| &service.mounts)
            .filter_map(|mount| match mount {
                PlanMount::Directory { source, .. } => Some(source.as_str()),
                // A shared folder already exists — the person chose it, and
                // `share_folder` refused it if it did not. The installer must
                // not create it, and must never treat it as its own to delete.
                PlanMount::Volume { .. } | PlanMount::Host { .. } => None,
            })
            .collect();
        directories.sort_unstable();
        directories.dedup();
        directories
    }

    pub fn validate(&self) -> Result<(), String> {
        if !crate::model::is_valid_installed_app_id(&self.id) {
            return Err("plan id is not a valid app id".into());
        }
        if self.services.is_empty() {
            return Err("a plan needs at least one service".into());
        }
        let mut names: Vec<&str> = self.services.iter().map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        if names.len() != unique {
            return Err("service names must be unique".into());
        }

        // Exactly one endpoint reaches the host. A database beside a web app
        // must stay on the internal network, and the launcher has exactly one
        // address to open.
        let published: Vec<_> = self
            .services
            .iter()
            .filter_map(|service| service.published)
            .collect();
        if published.len() != 1 {
            return Err(format!(
                "a plan must publish exactly one port, found {}",
                published.len()
            ));
        }
        if published[0].host < 1024 {
            return Err("published host port must be 1024 or above".into());
        }
        let companions = self.companions();
        if companions.len() > MAX_COMPANION_PORTS {
            return Err(format!(
                "a plan may publish at most {MAX_COMPANION_PORTS} second addresses, found {}",
                companions.len()
            ));
        }
        let mut hosts = vec![published[0].host];
        for (service, port) in &companions {
            // A service may answer on both: Gitea serves git over SSH on a
            // second address beside the pages it publishes.
            if service
                .published
                .is_some_and(|main| main.container == port.container)
            {
                return Err(format!(
                    "service {:?} publishes container port {} twice",
                    service.name, port.container
                ));
            }
            if port.host < 1024 || port.container == 0 {
                return Err(format!(
                    "second address for {:?} must use a host port of 1024 or above",
                    service.name
                ));
            }
            if hosts.contains(&port.host) {
                return Err(format!("host port {} is published twice", port.host));
            }
            hosts.push(port.host);
        }

        let mut declared_used = vec![false; self.named_volumes.len()];
        for volume in &self.named_volumes {
            if !is_plain_name(volume) {
                return Err(format!("named volume {volume:?} is not a plain name"));
            }
        }
        for service in &self.services {
            service.validate()?;
            for dependency in &service.overrides.completed_dependencies {
                if !service.depends_on.contains(dependency) {
                    return Err("completion condition refers to an undeclared dependency".into());
                }
                if service.overrides.healthy_dependencies.contains(dependency) {
                    return Err(format!(
                        "{:?} waits for {dependency:?} both to finish and to be healthy",
                        service.name
                    ));
                }
            }
            if self.is_job(&service.name) {
                // A job runs and exits. Publishing an address nobody can
                // reach once it has, or starting something beside it that
                // does not wait for it, are both mistakes worth refusing.
                if service.published.is_some() || service.companion.is_some() {
                    return Err(format!(
                        "one-shot job {:?} cannot publish an address",
                        service.name
                    ));
                }
                if let Some(other) = self.services.iter().find(|other| {
                    other.depends_on.contains(&service.name)
                        && !other
                            .overrides
                            .completed_dependencies
                            .contains(&service.name)
                }) {
                    return Err(format!(
                        "{:?} depends on one-shot job {:?} without waiting for it to finish",
                        other.name, service.name
                    ));
                }
            }
            for dependency in &service.overrides.healthy_dependencies {
                if !service.depends_on.contains(dependency) {
                    return Err("health condition refers to an undeclared dependency".into());
                }
                if self
                    .services
                    .iter()
                    .find(|s| &s.name == dependency)
                    .is_some_and(|s| {
                        s.overrides
                            .healthcheck
                            .as_ref()
                            .is_some_and(|h| h.test == Some(HealthTest::Disabled))
                    })
                {
                    return Err("health condition targets a disabled health check".into());
                }
            }
            for dependency in &service.depends_on {
                if dependency == &service.name {
                    return Err(format!("service {:?} depends on itself", service.name));
                }
                if !self.services.iter().any(|other| &other.name == dependency) {
                    return Err(format!(
                        "service {:?} depends on unknown service {dependency:?}",
                        service.name
                    ));
                }
            }
            for mount in &service.mounts {
                if let PlanMount::Volume { name, .. } = mount {
                    match self.named_volumes.iter().position(|v| v == name) {
                        Some(index) => declared_used[index] = true,
                        None => return Err(format!("mount uses undeclared volume {name:?}")),
                    }
                }
            }
        }
        let mut ordered = std::collections::BTreeSet::new();
        loop {
            let before = ordered.len();
            for service in &self.services {
                if service.depends_on.iter().all(|name| ordered.contains(name)) {
                    ordered.insert(service.name.clone());
                }
            }
            if ordered.len() == self.services.len() {
                break;
            }
            if ordered.len() == before {
                return Err("service dependency graph contains a cycle".into());
            }
        }
        if let Some(index) = declared_used.iter().position(|used| !used) {
            return Err(format!(
                "named volume {:?} is declared but never mounted",
                self.named_volumes[index]
            ));
        }
        self.validate_networks()
    }

    /// Networks are declared once, joined by name, and never the only one
    /// under an address: a port published from an internal network goes
    /// nowhere.
    fn validate_networks(&self) -> Result<(), String> {
        let mut declared = std::collections::BTreeSet::new();
        for network in &self.internal_networks {
            if !is_plain_name(network) || network == "default" {
                return Err(format!("network {network:?} is not a plain name"));
            }
            if !declared.insert(network.as_str()) {
                return Err(format!("network {network:?} is declared twice"));
            }
        }
        let mut joined = std::collections::BTreeSet::new();
        for service in &self.services {
            let mut seen = std::collections::BTreeSet::new();
            for network in &service.networks {
                if network != "default" && !declared.contains(network.as_str()) {
                    return Err(format!(
                        "service {:?} joins undeclared network {network:?}",
                        service.name
                    ));
                }
                if !seen.insert(network.as_str()) {
                    return Err(format!(
                        "service {:?} joins network {network:?} twice",
                        service.name
                    ));
                }
                joined.insert(network.as_str());
            }
            let on_default =
                service.networks.is_empty() || service.networks.iter().any(|n| n == "default");
            if !on_default && (service.published.is_some() || service.companion.is_some()) {
                return Err(format!(
                    "service {:?} publishes an address but is not on the default network",
                    service.name
                ));
            }
        }
        if let Some(unused) = declared.iter().find(|network| !joined.contains(*network)) {
            return Err(format!(
                "network {unused:?} is declared but nothing joins it"
            ));
        }
        Ok(())
    }

    /// Render the Compose file this plan describes.
    pub fn to_compose(&self) -> Result<String, String> {
        self.to_compose_with_mounts(|mount| Ok(format!("      - {}\n", mount.rendered())))
    }

    /// Share the complete renderer with engine-specific mount projection.
    pub(crate) fn to_compose_with_mounts(
        &self,
        mut render_mount: impl FnMut(&PlanMount) -> Result<String, String>,
    ) -> Result<String, String> {
        self.validate()?;
        let mut out = String::from("services:\n");
        for service in &self.services {
            out.push_str(&format!("  {}:\n", service.name));
            match &service.digest {
                Some(digest) => out.push_str(&format!("    image: {}@{}\n", service.image, digest)),
                None => out.push_str(&format!("    image: {}\n", service.image)),
            }
            out.push_str(&format!(
                "    container_name: {}\n",
                self.container_name(service)
            ));
            // A job that restarted itself would run its migration forever.
            if self.is_job(&service.name) {
                out.push_str("    restart: \"no\"\n");
            } else {
                out.push_str("    restart: unless-stopped\n");
            }
            // One `ports:` for both, because a service may answer on its own
            // address and a second one: Gitea serves git over SSH beside its
            // pages, and two `ports:` keys are a Compose parse error.
            if service.published.is_some() || service.companion.is_some() {
                out.push_str("    ports:\n");
                if let Some(port) = service.published {
                    out.push_str(&format!(
                        "      - \"127.0.0.1:{}:{}\"\n",
                        port.host, port.container
                    ));
                }
                // The long form, so the main address stays the only
                // short-form line and `published_host_port` can never pick up
                // a second one.
                if let Some(port) = service.companion {
                    out.push_str(&format!(
                        "      - target: {}\n        published: \"{}\"\n        host_ip: 127.0.0.1\n",
                        port.container, port.host
                    ));
                }
            }
            if !service.environment.is_empty() {
                out.push_str("    environment:\n");
                for (key, value) in &service.environment {
                    out.push_str(&format!("      {key}: {}\n", scalar(value)));
                }
            }
            if !service.mounts.is_empty() {
                out.push_str("    volumes:\n");
                for mount in &service.mounts {
                    out.push_str(&render_mount(mount)?);
                }
            }
            if !service.depends_on.is_empty() {
                out.push_str("    depends_on:\n");
                let conditional = !service.overrides.healthy_dependencies.is_empty()
                    || !service.overrides.completed_dependencies.is_empty();
                for dependency in &service.depends_on {
                    if !conditional {
                        out.push_str(&format!("      - {dependency}\n"));
                    } else {
                        let condition =
                            if service.overrides.healthy_dependencies.contains(dependency) {
                                "service_healthy"
                            } else if service
                                .overrides
                                .completed_dependencies
                                .contains(dependency)
                            {
                                "service_completed_successfully"
                            } else {
                                "service_started"
                            };
                        out.push_str(&format!(
                            "      {dependency}:\n        condition: {condition}\n"
                        ));
                    }
                }
            }
            if !service.networks.is_empty() {
                out.push_str("    networks:\n");
                for network in &service.networks {
                    out.push_str(&format!("      - {network}\n"));
                }
            }
            service.overrides.render(&mut out);
        }
        if !self.named_volumes.is_empty() {
            out.push_str("volumes:\n");
            for volume in &self.named_volumes {
                out.push_str(&format!("  {volume}:\n"));
            }
        }
        if !self.internal_networks.is_empty() {
            out.push_str("networks:\n");
            for network in &self.internal_networks {
                out.push_str(&format!("  {network}:\n    internal: true\n"));
            }
        }
        Ok(out)
    }
}

impl PlanService {
    fn validate(&self) -> Result<(), String> {
        self.overrides.validate()?;
        if !is_plain_name(&self.name) {
            return Err(format!("service name {:?} is not a plain name", self.name));
        }
        let Some((_, tag)) = self.image.rsplit_once(':') else {
            return Err(format!("image {:?} is not pinned to a tag", self.image));
        };
        if matches!(tag, "latest" | "stable" | "main" | "edge") || tag.is_empty() {
            return Err(format!(
                "image {:?} is not pinned to a fixed tag",
                self.image
            ));
        }
        if self
            .image
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err("image contains whitespace or control characters".into());
        }
        if let Some(digest) = &self.digest {
            let hex = digest.strip_prefix("sha256:").unwrap_or_default();
            if hex.len() != 64
                || !hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(format!(
                    "digest {digest:?} for {:?} is not a sha256 digest",
                    self.image
                ));
            }
        }
        for (key, value) in &self.environment {
            // Uppercase is a convention, not a rule. Ghost configures itself
            // with `database__client`, Jellyfin with
            // `JELLYFIN_PublishedServerUrl`, and both are ordinary environment
            // names that Docker accepts — requiring SHOUTING_CASE refused four
            // real apps for a house style. What actually matters is that the
            // name cannot break out of the `key: value` it renders into.
            let first_is_name_like = key
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if !first_is_name_like
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                return Err(format!(
                    "environment key {key:?} is not a plain environment name"
                ));
            }
            if value.chars().any(|c| c.is_control()) {
                return Err(format!(
                    "environment value for {key:?} has control characters"
                ));
            }
        }
        for mount in &self.mounts {
            let target = mount.target();
            if !target.starts_with('/') || target.contains("..") || target.contains('\\') {
                return Err(format!(
                    "mount target {target:?} must be an absolute container path"
                ));
            }
            match mount {
                PlanMount::Directory { source, .. } => {
                    if source.is_empty()
                        || source.starts_with('/')
                        || source.contains("..")
                        || source.contains('\\')
                        || source.contains(':')
                    {
                        return Err(format!(
                            "mount source {source:?} must stay inside the project directory"
                        ));
                    }
                }
                PlanMount::Host { source, .. } => {
                    // Before resolution this must be a placeholder and nothing
                    // else. A plan carrying a literal path would be a folder
                    // nobody was asked about; after resolution the path has
                    // already been through `share_folder`.
                    let placeholder = source.starts_with("${")
                        && source.ends_with('}')
                        && !source[2..source.len() - 1].contains(['$', '{', '}']);
                    if !placeholder && !is_resolved_host_path(source) {
                        return Err(format!(
                            "host mount source {source:?} must be a declared folder answer"
                        ));
                    }
                }
                PlanMount::Volume { .. } => {}
            }
        }
        Ok(())
    }
}

/// Whether a host mount source is an absolute path a person's answer produced.
///
/// Deliberately narrow: no relative segments, no quotes, and nothing that could
/// turn one mount argument into two.
fn is_resolved_host_path(source: &str) -> bool {
    let bytes = source.as_bytes();
    let absolute = source.starts_with('/')
        || (bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && &source[1..3] == ":\\");
    absolute
        && !source.contains("..")
        && !source
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\'')
}

fn is_plain_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

/// Quote a value only when leaving it bare would change its YAML type.
///
/// `sqlite` is a string either way; `5230` and `true` would become a number and
/// a boolean, which Compose rejects for environment values.
fn scalar(value: &str) -> String {
    let bare_is_a_string = !value.is_empty()
        && value.parse::<i64>().is_err()
        && value.parse::<f64>().is_err()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
        )
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        && !value.starts_with('-');
    if bare_is_a_string {
        value.to_owned()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// Choose a host port that nothing is listening on.
///
/// The recipe's own port is tried first so an ordinary install keeps the
/// documented address; the scan only matters when something else already holds
/// it. Ports are not reserved, so a caller still has to handle the publish
/// failing — this narrows the window, it does not remove it.
pub fn choose_free_port(
    probe: &dyn crate::runtime::PortProbe,
    preferred: u16,
    attempts: u16,
) -> Result<u16, String> {
    if preferred >= 1024 && probe.available(preferred) {
        return Ok(preferred);
    }
    let start = preferred.max(1024);
    for offset in 1..=attempts {
        let candidate = start.checked_add(offset).filter(|port| *port >= 1024);
        if let Some(port) = candidate {
            if probe.available(port) {
                return Ok(port);
            }
        }
    }
    Err(format!(
        "no free port found near {preferred}; stop whatever is using them and try again"
    ))
}

/// The host port a previously rendered Compose file publishes.
///
/// A reinstall has to land on the address the app already had. After a
/// keep-data uninstall the registry entry is gone, so the retained Compose
/// file beside the preserved data is the only record of it left.
pub fn published_host_port(compose: &str) -> Option<u16> {
    compose.lines().map(str::trim).find_map(|line| {
        let value = line.strip_prefix("- \"127.0.0.1:")?.strip_suffix('\"')?;
        value.split_once(':')?.0.parse().ok()
    })
}

/// The second addresses a previously rendered Compose file publishes, by
/// service.
///
/// A reinstall keeps them for the same reason it keeps the main one: an app
/// may have stored links to its object store, and moving the port would break
/// every one of them.
pub fn companion_host_ports(compose: &str) -> std::collections::BTreeMap<String, u16> {
    let mut found = std::collections::BTreeMap::new();
    let mut service: Option<&str> = None;
    for line in compose.lines() {
        if let Some(name) = line
            .strip_prefix("  ")
            .filter(|rest| !rest.starts_with(' '))
            .and_then(|rest| rest.strip_suffix(':'))
        {
            service = Some(name);
            continue;
        }
        if !line.starts_with(' ') {
            service = None;
            continue;
        }
        let Some(current) = service else { continue };
        if let Some(port) = line
            .trim()
            .strip_prefix("published: \"")
            .and_then(|rest| rest.strip_suffix('"'))
            .and_then(|port| port.parse().ok())
        {
            found.insert(current.to_owned(), port);
        }
    }
    found
}

/// Express an existing reviewed recipe as a plan.
///
/// This is the bridge that proves the model covers what already ships. Recipes
/// still install from their own Compose file; nothing here changes that.
pub fn plan_for_recipe(recipe: &Recipe) -> Result<DeploymentPlan, String> {
    let compose = &recipe.compose;
    let named_volumes: Vec<String> = compose
        .split_once("\nvolumes:\n")
        .map(|(_, tail)| {
            tail.lines()
                .take_while(|line| line.starts_with("  ") && line.trim_end().ends_with(':'))
                .map(|line| line.trim().trim_end_matches(':').to_owned())
                .collect()
        })
        .unwrap_or_default();

    let mut environment = Vec::new();
    if let Some((_, tail)) = compose.split_once("    environment:\n") {
        for line in tail.lines() {
            let Some(rest) = line.strip_prefix("      ") else {
                break;
            };
            let Some((key, value)) = rest.split_once(": ") else {
                break;
            };
            environment.push((
                key.trim().to_owned(),
                value.trim().trim_matches('"').to_owned(),
            ));
        }
    }

    let mounts = compose
        .split_once("    volumes:\n")
        .map(|(_, tail)| {
            tail.lines()
                .take_while(|line| line.starts_with("      - "))
                .filter_map(|line| line.trim_start_matches("      - ").split_once(':'))
                .map(|(source, target)| {
                    // `./data:/var/lib/app:ro` splits into a target that still
                    // carries the flag, so it is taken off here rather than
                    // becoming part of the path.
                    let (target, read_only) = match target.strip_suffix(":ro") {
                        Some(path) => (path, true),
                        None => (target, false),
                    };
                    if let Some(directory) = source.strip_prefix("./") {
                        PlanMount::Directory {
                            source: directory.to_owned(),
                            target: target.to_owned(),
                            read_only,
                        }
                    } else {
                        PlanMount::Volume {
                            name: source.to_owned(),
                            target: target.to_owned(),
                            read_only,
                        }
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let plan = DeploymentPlan {
        id: recipe.id.clone(),
        services: vec![PlanService {
            name: recipe.id.clone(),
            image: recipe.image.clone(),
            digest: Some(recipe.requirements.image_audit.index_digest.clone()),
            environment,
            companion: None,
            networks: Vec::new(),
            published: Some(PublishedPort {
                host: recipe.host_port,
                container: recipe.container_port,
            }),
            mounts,
            depends_on: Vec::new(),
            overrides: PlanOverrides::default(),
        }],
        named_volumes,
        internal_networks: Vec::new(),
    };
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uppercase is a convention. Ghost and Jellyfin both configure themselves
    /// with names that are not, and refusing them was a house style masquerading
    /// as a safety rule.
    #[test]
    fn an_ordinary_environment_name_is_accepted_whatever_its_case() {
        for key in [
            "DATABASE_URL",
            "database__client",
            "JELLYFIN_PublishedServerUrl",
            "APP_CONFIG_auth_password",
            "_LEADING_UNDERSCORE",
            "log.level",
        ] {
            let mut plan = plan_with_environment(key, "value");
            assert!(
                plan.validate().is_ok(),
                "{key} was refused: {:?}",
                plan.validate()
            );
            // And it survives rendering as its own line.
            let compose = plan.to_compose().expect("it should render");
            assert!(compose.contains(key), "{compose}");
            plan.services[0].environment.clear();
        }
    }

    /// What the rule is actually for: a name that could stop being a name.
    #[test]
    fn an_environment_name_that_could_break_its_own_line_is_refused() {
        for key in [
            "",
            "9LEADING_DIGIT",
            "HAS SPACE",
            "HAS=EQUALS",
            "HAS
NEWLINE",
            "HAS$DOLLAR",
            "HAS\"QUOTE",
        ] {
            let plan = plan_with_environment(key, "value");
            assert!(plan.validate().is_err(), "{key:?} was accepted");
        }
    }

    fn plan_with_environment(key: &str, value: &str) -> DeploymentPlan {
        DeploymentPlan {
            id: "example".into(),
            services: vec![PlanService {
                name: "example".into(),
                image: "example/app:1.0".into(),
                digest: None,
                environment: vec![(key.to_owned(), value.to_owned())],
                companion: None,
                networks: Vec::new(),
                published: Some(PublishedPort {
                    host: 8080,
                    container: 80,
                }),
                mounts: Vec::new(),
                depends_on: Vec::new(),
                overrides: PlanOverrides::default(),
            }],
            named_volumes: Vec::new(),
            internal_networks: Vec::new(),
        }
    }

    use crate::recipes::reviewed_recipes;

    /// The gate for this model: it must express what already ships, exactly.
    #[test]
    fn every_shipped_recipe_renders_byte_for_byte_from_a_plan() {
        for recipe in reviewed_recipes() {
            let plan = plan_for_recipe(&recipe).expect("recipe should convert to a plan");
            assert_eq!(
                plan.to_compose().expect("plan should render"),
                recipe.compose,
                "rendered Compose differs for {}",
                recipe.id
            );
            let (service, port) = plan.published().expect("a published endpoint");
            assert_eq!(port.host, recipe.host_port);
            assert_eq!(port.container, recipe.container_port);
            assert_eq!(service.name, recipe.id);
            assert_eq!(plan.data_directories(), recipe.data_directories);
        }
    }

    #[test]
    fn resource_ceilings_render_only_when_explicit_and_reject_zero() {
        let mut plan = plan_with_environment("PORT", "80");
        let defaults = plan.to_compose().unwrap();
        assert!(!defaults.contains("mem_limit:"));
        assert!(!defaults.contains("cpus:"));
        assert!(!defaults.contains("pids_limit:"));

        let limits = &mut plan.services[0].overrides;
        limits.memory_limit_bytes = Some(512 * 1024 * 1024);
        limits.cpu_limit_millicores = Some(1250);
        limits.pids_limit = Some(256);
        let compose = plan.to_compose().unwrap();
        assert!(compose.contains("    mem_limit: 536870912\n"));
        assert!(compose.contains("    cpus: \"1.250\"\n"));
        assert!(compose.contains("    pids_limit: 256\n"));

        plan.services[0].overrides.memory_limit_bytes = Some(0);
        assert!(plan.validate().is_err());
        plan.services[0].overrides.memory_limit_bytes = None;
        plan.services[0].overrides.cpu_limit_millicores = Some(0);
        assert!(plan.validate().is_err());
        plan.services[0].overrides.cpu_limit_millicores = None;
        plan.services[0].overrides.pids_limit = Some(0);
        assert!(plan.validate().is_err());
    }

    fn web() -> PlanService {
        PlanService {
            name: "web".into(),
            image: "example/web:1.2.3".into(),
            digest: None,
            environment: vec![("DATABASE_HOST".into(), "db".into())],
            companion: None,
            networks: Vec::new(),
            published: Some(PublishedPort {
                host: 8080,
                container: 8080,
            }),
            mounts: vec![PlanMount::directory("data", "/var/lib/web")],
            depends_on: vec!["db".into()],
            overrides: PlanOverrides::default(),
        }
    }
    fn database() -> PlanService {
        PlanService {
            name: "db".into(),
            image: "example/postgres:16.2".into(),
            digest: None,
            environment: vec![("POSTGRES_DB".into(), "app".into())],
            companion: None,
            networks: Vec::new(),
            published: None,
            mounts: vec![PlanMount::Volume {
                name: "db-data".into(),
                target: "/var/lib/postgresql/data".into(),
                read_only: false,
            }],
            depends_on: Vec::new(),
            overrides: PlanOverrides::default(),
        }
    }
    fn pair() -> DeploymentPlan {
        DeploymentPlan {
            id: "example".into(),
            services: vec![web(), database()],
            named_volumes: vec!["db-data".into()],
            internal_networks: Vec::new(),
        }
    }

    #[test]
    fn a_web_app_beside_a_database_renders_without_hand_editing() {
        let rendered = pair().to_compose().expect("pair should render");
        assert_eq!(
            rendered,
            "services:\n  \
             web:\n    image: example/web:1.2.3\n    container_name: local-store-example-web\n    \
             restart: unless-stopped\n    ports:\n      - \"127.0.0.1:8080:8080\"\n    \
             environment:\n      DATABASE_HOST: db\n    volumes:\n      - ./data:/var/lib/web\n    \
             depends_on:\n      - db\n  \
             db:\n    image: example/postgres:16.2\n    container_name: local-store-example-db\n    \
             restart: unless-stopped\n    environment:\n      POSTGRES_DB: app\n    \
             volumes:\n      - db-data:/var/lib/postgresql/data\nvolumes:\n  db-data:\n"
        );
        // The database is reachable from the web service and from nowhere else.
        assert!(!rendered.contains("127.0.0.1:5432"));
    }

    #[test]
    fn a_plan_that_would_expose_or_confuse_the_host_is_refused() {
        // Two published ports: the launcher would not know what to open, and a
        // database would be reachable from the host.
        let mut two = pair();
        two.services[1].published = Some(PublishedPort {
            host: 5432,
            container: 5432,
        });
        assert!(two.to_compose().unwrap_err().contains("exactly one port"));

        // No published port at all.
        let mut none = pair();
        none.services[0].published = None;
        assert!(none.to_compose().unwrap_err().contains("exactly one port"));

        // A privileged host port.
        let mut privileged = pair();
        privileged.services[0].published = Some(PublishedPort {
            host: 80,
            container: 8080,
        });
        assert!(privileged.to_compose().unwrap_err().contains("1024"));
    }

    #[test]
    fn a_read_only_mount_renders_the_flag_and_still_gets_its_directory_created() {
        let mut plan = pair();
        plan.services[0].mounts = vec![
            PlanMount::directory("data", "/var/lib/web"),
            PlanMount::Directory {
                source: "config".into(),
                target: "/etc/web".into(),
                read_only: true,
            },
        ];
        let compose = plan.to_compose().unwrap();
        assert!(
            compose.contains(
                "      - ./data:/var/lib/web
"
            ),
            "{compose}"
        );
        assert!(
            compose.contains(
                "      - ./config:/etc/web:ro
"
            ),
            "{compose}"
        );
        // The installer creates bind sources before starting; a read-only
        // mount whose directory is missing would have Docker create it as root.
        assert_eq!(plan.data_directories(), vec!["config", "data"]);
    }

    #[test]
    fn a_plan_that_would_reach_outside_its_project_is_refused() {
        for source in ["../escape", "/etc", "data\\sneaky", "C:/data"] {
            let mut plan = pair();
            plan.services[0].mounts = vec![PlanMount::directory(source, "/var/lib/web")];
            assert!(
                plan.to_compose().is_err(),
                "mount source {source:?} was accepted"
            );
        }
        let mut relative_target = pair();
        relative_target.services[0].mounts = vec![PlanMount::directory("data", "var/lib/web")];
        assert!(relative_target.to_compose().is_err());
    }

    /// An offered app installs the manifest list that was audited, not
    /// whatever its tag points at on the day somebody installs it.
    #[test]
    fn an_audited_digest_is_what_the_compose_file_pulls() {
        let mut service = web();
        service.depends_on.clear();
        service.digest =
            Some("sha256:abababababababababababababababababababababababababababababababab".into());
        let plan = DeploymentPlan {
            id: "example".into(),
            services: vec![service],
            named_volumes: Vec::new(),
            internal_networks: Vec::new(),
        };
        let compose = plan.to_compose().unwrap();
        assert!(
            compose.contains("image: example/web:1.2.3@sha256:abababababababababababababababababababababababababababababababab"),
            "{compose}"
        );

        let mut broken = plan.clone();
        broken.services[0].digest = Some("sha256:not-a-digest".into());
        assert!(
            broken.to_compose().is_err(),
            "a malformed digest was rendered"
        );
    }

    #[test]
    fn unpinned_images_and_dangling_volumes_are_refused() {
        for image in ["example/web:latest", "example/web:stable", "example/web"] {
            let mut plan = pair();
            plan.services[0].image = image.into();
            assert!(plan.to_compose().is_err(), "image {image:?} was accepted");
        }
        let mut undeclared = pair();
        undeclared.named_volumes.clear();
        assert!(undeclared.to_compose().unwrap_err().contains("undeclared"));

        let mut unused = pair();
        unused.named_volumes.push("orphan".into());
        assert!(unused.to_compose().unwrap_err().contains("never mounted"));
    }

    #[test]
    fn dependencies_must_name_a_service_in_the_plan() {
        let mut missing = pair();
        missing.services[0].depends_on = vec!["cache".into()];
        assert!(missing
            .to_compose()
            .unwrap_err()
            .contains("unknown service"));

        let mut itself = pair();
        itself.services[0].depends_on = vec!["web".into()];
        assert!(itself
            .to_compose()
            .unwrap_err()
            .contains("depends on itself"));
    }

    struct Ports(Vec<u16>);
    impl crate::runtime::PortProbe for Ports {
        fn available(&self, port: u16) -> bool {
            self.0.contains(&port)
        }
    }

    #[test]
    fn health_dependencies_cannot_dangle_disable_or_cycle() {
        let mut plan = pair();
        plan.services[0]
            .overrides
            .healthy_dependencies
            .insert("db".into());
        assert!(plan.validate().is_ok());
        plan.services[1].overrides.healthcheck = Some(PlanHealthcheck {
            test: Some(HealthTest::Disabled),
            ..Default::default()
        });
        assert!(plan.validate().unwrap_err().contains("disabled"));
        plan.services[1].overrides.healthcheck = None;
        plan.services[1].depends_on.push("web".into());
        assert!(plan.validate().unwrap_err().contains("cycle"));
        plan.services[1].depends_on.clear();
        plan.services[0]
            .overrides
            .healthy_dependencies
            .insert("missing".into());
        assert!(plan.validate().unwrap_err().contains("undeclared"));
    }

    /// A migration that runs once and exits, which the app waits for.
    fn with_migration() -> DeploymentPlan {
        let mut plan = pair();
        let mut migrate = database();
        migrate.name = "migrate".into();
        migrate.mounts.clear();
        migrate.depends_on = vec!["db".into()];
        plan.services.push(migrate);
        plan.services[0].depends_on.push("migrate".into());
        plan.services[0]
            .overrides
            .completed_dependencies
            .insert("migrate".into());
        plan
    }

    #[test]
    fn a_one_shot_job_runs_once_and_the_app_waits_for_it_to_finish() {
        let plan = with_migration();
        assert!(plan.is_job("migrate"));
        assert!(!plan.is_job("db"));
        let rendered = plan.to_compose().expect("a job should render");
        let migrate = rendered.split("\n  migrate:\n").nth(1).unwrap();
        assert!(
            migrate.contains("    restart: \"no\"\n"),
            "a job that restarts runs its migration forever:\n{rendered}"
        );
        assert!(rendered
            .contains("      migrate:\n        condition: service_completed_successfully\n"));
        // Everything else waits the ordinary way.
        assert!(rendered.contains("      db:\n        condition: service_started\n"));
        assert_eq!(rendered.matches("restart: unless-stopped").count(), 2);
    }

    #[test]
    fn a_job_is_refused_when_it_publishes_or_something_does_not_wait_for_it() {
        let mut published = with_migration();
        published.services[2].companion = Some(PublishedPort {
            host: 9000,
            container: 9000,
        });
        assert!(published.validate().unwrap_err().contains("cannot publish"));

        // A second service that only waits for the job to start would race it.
        let mut racing = with_migration();
        racing.services[1].depends_on.push("migrate".into());
        assert!(racing.validate().unwrap_err().contains("without waiting"));

        let mut undeclared = with_migration();
        undeclared.services[0]
            .depends_on
            .retain(|name| name != "migrate");
        assert!(undeclared.validate().unwrap_err().contains("undeclared"));

        let mut both = with_migration();
        both.services[0]
            .overrides
            .healthy_dependencies
            .insert("migrate".into());
        assert!(both.validate().unwrap_err().contains("both to finish"));
    }

    /// A realtime server the app's own pages call, beside the main address.
    fn with_companion() -> DeploymentPlan {
        let mut plan = pair();
        let mut realtime = database();
        realtime.name = "realtime".into();
        realtime.mounts.clear();
        realtime.companion = Some(PublishedPort {
            host: 3002,
            container: 3002,
        });
        // Listed first on purpose: the main address must still be the one a
        // retained file is read back as.
        plan.services.insert(0, realtime);
        plan
    }

    #[test]
    fn a_second_address_is_published_on_loopback_and_never_mistaken_for_the_app() {
        let plan = with_companion();
        plan.validate().expect("one second address is allowed");
        let rendered = plan.to_compose().unwrap();
        assert!(rendered.contains(
            "      - target: 3002\n        published: \"3002\"\n        host_ip: 127.0.0.1\n"
        ));
        assert_eq!(published_host_port(&rendered), Some(8080));
        assert_eq!(
            companion_host_ports(&rendered),
            [("realtime".to_owned(), 3002)].into_iter().collect()
        );
        assert_eq!(plan.published().unwrap().0.name, "web");

        let mut moved = plan.clone();
        moved.set_companion_host("realtime", 43002);
        assert_eq!(moved.companions()[0].1.host, 43002);
    }

    #[test]
    fn second_addresses_are_few_distinct_and_never_the_main_one() {
        let mut clash = with_companion();
        clash.services[0].companion = Some(PublishedPort {
            host: 8080,
            container: 3002,
        });
        assert!(clash.validate().unwrap_err().contains("published twice"));

        // A service may answer on both, as Gitea does over SSH beside the
        // pages it publishes — but never twice on one container port.
        let mut both = with_companion();
        both.services[1].companion = Some(PublishedPort {
            host: 9000,
            container: 22,
        });
        both.validate()
            .expect("a main address and a second one may share a service");
        // Two `ports:` keys on one service is a Compose parse error, which is
        // how Gitea first failed.
        let rendered = both.to_compose().expect("both addresses should render");
        let web = rendered.split("\n  web:\n").nth(1).unwrap();
        let web = web.split("\n  db:\n").next().unwrap();
        assert_eq!(web.matches("ports:").count(), 1, "{rendered}");
        assert!(web.contains("- target: 22\n"), "{rendered}");

        let mut twice = with_companion();
        twice.services[1].companion = Some(PublishedPort {
            host: 9000,
            container: 8080,
        });
        assert!(twice.validate().unwrap_err().contains("twice"));

        let mut privileged = with_companion();
        privileged.services[0].companion = Some(PublishedPort {
            host: 80,
            container: 80,
        });
        assert!(privileged.validate().unwrap_err().contains("1024"));

        let mut many = with_companion();
        for (index, host) in [9001, 9002].into_iter().enumerate() {
            let mut extra = database();
            extra.name = format!("extra{index}");
            extra.mounts.clear();
            extra.companion = Some(PublishedPort {
                host,
                container: host,
            });
            many.services.push(extra);
        }
        assert!(many.validate().unwrap_err().contains("at most"));
    }

    /// Dify's shape: a sandbox that reaches nothing but its proxy, and a
    /// proxy that sits on both sides.
    fn with_sandbox() -> DeploymentPlan {
        let mut plan = pair();
        let mut sandbox = database();
        sandbox.name = "sandbox".into();
        sandbox.mounts.clear();
        sandbox.networks = vec!["isolated".into()];
        let mut proxy = database();
        proxy.name = "proxy".into();
        proxy.mounts.clear();
        proxy.networks = vec!["default".into(), "isolated".into()];
        plan.services.push(sandbox);
        plan.services.push(proxy);
        plan.internal_networks = vec!["isolated".into()];
        plan
    }

    #[test]
    fn an_internal_network_renders_and_keeps_its_members_off_the_default_one() {
        let plan = with_sandbox();
        let rendered = plan
            .to_compose()
            .expect("an internal network should render");
        assert!(
            rendered.ends_with("networks:\n  isolated:\n    internal: true\n"),
            "{rendered}"
        );
        let sandbox = rendered.split("\n  sandbox:\n").nth(1).unwrap();
        let sandbox = sandbox.split("\n  proxy:\n").next().unwrap();
        assert!(
            sandbox.ends_with("    networks:\n      - isolated"),
            "{rendered}"
        );
        assert!(!sandbox.contains("- default"), "{rendered}");
        // A service that names no network stays on the project's own, unchanged.
        let web = rendered.split("\n  db:\n").next().unwrap();
        assert!(!web.contains("networks:"), "{rendered}");
    }

    #[test]
    fn a_network_is_declared_joined_and_never_the_only_one_under_an_address() {
        let mut undeclared = with_sandbox();
        undeclared.internal_networks.clear();
        assert!(undeclared
            .validate()
            .unwrap_err()
            .contains("undeclared network"));

        let mut unused = with_sandbox();
        unused.internal_networks.push("spare".into());
        assert!(unused.validate().unwrap_err().contains("nothing joins it"));

        let mut twice = with_sandbox();
        twice.internal_networks.push("isolated".into());
        assert!(twice.validate().unwrap_err().contains("declared twice"));

        let mut renamed = with_sandbox();
        renamed.internal_networks = vec!["default".into()];
        assert!(renamed.validate().is_err());

        // The address would lead nowhere.
        let mut stranded = with_sandbox();
        stranded.services[0].networks = vec!["isolated".into()];
        assert!(stranded
            .validate()
            .unwrap_err()
            .contains("not on the default network"));
        let mut companion = with_sandbox();
        companion.services[2].companion = Some(PublishedPort {
            host: 9000,
            container: 9000,
        });
        assert!(companion
            .validate()
            .unwrap_err()
            .contains("not on the default network"));
    }

    #[test]
    fn a_busy_port_moves_aside_without_disturbing_a_free_one() {
        // The documented port is kept when it is free.
        assert_eq!(
            choose_free_port(&Ports(vec![5230]), 5230, 20).unwrap(),
            5230
        );
        // Otherwise the next free one nearby is taken.
        assert_eq!(
            choose_free_port(&Ports(vec![5233]), 5230, 20).unwrap(),
            5233
        );
        // Privileged ports are never chosen, even if asked for.
        assert_eq!(
            choose_free_port(&Ports(vec![80, 1030]), 80, 20).unwrap(),
            1030
        );
        // Nothing free nearby is an error, not a silent bad port.
        assert!(choose_free_port(&Ports(Vec::new()), 5230, 5).is_err());
    }
}
