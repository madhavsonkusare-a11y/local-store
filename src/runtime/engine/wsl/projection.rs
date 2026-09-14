//! Render a separate Linux Compose artifact from a resolved, validated plan.
//! Mapping paths is not permission to share them: folder consent/confinement
//! remains the installer's responsibility, as does checking the WSL mount layout.
use super::windows_drive_path;
use crate::{
    error::{AppError, AppResult},
    plan::{DeploymentPlan, PlanMount},
    setup::{is_confined_seed_path, SeedFile, MAX_SEEDS, MAX_SEED_BYTES},
};

pub const COMPOSE_FILE: &str = "compose.wsl.yaml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPair {
    pub windows: String,
    pub linux: String,
}
impl PathPair {
    fn new(windows: String) -> AppResult<Self> {
        let linux = windows_drive_path(&windows)?;
        Ok(Self { windows, linux })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedSeed {
    pub path: PathPair,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedPlan {
    pub project: PathPair,
    pub compose_file: PathPair,
    pub compose: String,
    /// Bind identities for recovery/ownership checks; excludes named volumes.
    pub binds: Vec<PathPair>,
    pub seeds: Vec<ProjectedSeed>,
}

/// Pure projection: writes nothing and never changes the original plan or seeds.
/// Absolute Linux bind sources prevent Compose's location from changing storage.
pub fn project_plan(
    plan: &DeploymentPlan,
    windows_project: &str,
    seeds: &[SeedFile],
) -> AppResult<ProjectedPlan> {
    let project = PathPair::new(windows_project.to_owned())?;
    let child = |relative: &str| {
        PathPair::new(format!(
            "{}/{}",
            windows_project.trim_end_matches(['/', '\\']),
            relative
        ))
    };
    let compose_file = child(COMPOSE_FILE)?;
    if seeds.len() > MAX_SEEDS {
        return Err(AppError::invalid("Too many seed files for WSL projection."));
    }
    let mut projected_seeds = Vec::new();
    let mut seed_paths = std::collections::BTreeSet::new();
    for seed in seeds {
        if !is_confined_seed_path(&seed.path)
            || seed.content.len() > MAX_SEED_BYTES
            || !seed_paths.insert(seed.path.to_ascii_lowercase())
        {
            return Err(AppError::invalid(
                "Invalid or duplicate seed file for WSL projection.",
            ));
        }
        projected_seeds.push(ProjectedSeed {
            path: child(&seed.path)?,
            content: seed.content.clone(),
        });
    }
    let mut binds = Vec::new();
    let compose = plan.to_compose_with_mounts(|mount| {
        let (kind, source, target, read_only) = match mount {
            PlanMount::Directory { source, target, read_only } => {
                let pair = child(source).map_err(|e| e.message)?;
                let linux = pair.linux.clone(); binds.push(pair);
                ("bind", linux, target, read_only)
            }
            PlanMount::Host { source, target, read_only } => {
                let pair = PathPair::new(source.clone()).map_err(|e| e.message)?;
                let linux = pair.linux.clone(); binds.push(pair);
                ("bind", linux, target, read_only)
            }
            PlanMount::Volume { name, target, read_only } => ("volume", name.clone(), target, read_only),
        };
        // JSON strings are YAML strings. Compose still interpolates quoted '$',
        // so escape dollars independently of YAML quoting, including targets.
        let quote = |value: &str| serde_json::to_string(&value.replace('$', "$$")).map_err(|e| e.to_string());
        let mut rendered = format!("      - type: {kind}\n        source: {}\n        target: {}\n        read_only: {read_only}\n", quote(&source)?, quote(target)?);
        if kind == "bind" { rendered.push_str("        bind:\n          create_host_path: false\n"); }
        Ok(rendered)
    }).map_err(AppError::invalid)?;
    Ok(ProjectedPlan {
        project,
        compose_file,
        compose,
        binds,
        seeds: projected_seeds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> DeploymentPlan {
        crate::plan::plan_for_recipe(&crate::recipes::recipe("memos").unwrap()).unwrap()
    }

    #[test]
    fn projects_bind_and_seed_paths_but_preserves_the_original_plan() {
        let mut plan = plan();
        plan.services[0].mounts.push(PlanMount::Host {
            source: r"E:\Photos # ${HOME}".into(),
            target: "/photos".into(),
            read_only: true,
        });
        let before = plan.clone();
        let seed = SeedFile {
            path: "data/config.json".into(),
            content: "{\"path\":\"C:/literal\"}".into(),
        };
        let projected =
            project_plan(&plan, r"D:\My Vault\memos", std::slice::from_ref(&seed)).unwrap();
        assert_eq!(plan, before);
        assert_eq!(projected.binds[1].linux, "/mnt/e/Photos # ${HOME}");
        assert!(projected.compose.contains("$${HOME}"));
        assert!(projected.compose.contains("create_host_path: false"));
        assert_eq!(projected.seeds[0].content, seed.content);
        assert_eq!(
            projected.seeds[0].path.linux,
            "/mnt/d/My Vault/memos/data/config.json"
        );
        assert_eq!(
            projected.compose_file.linux,
            "/mnt/d/My Vault/memos/compose.wsl.yaml"
        );
    }

    #[test]
    fn refuses_unresolved_host_mounts_and_invalid_seed_paths() {
        let mut plan = plan();
        plan.services[0].mounts.push(PlanMount::Host {
            source: "${FOLDER}".into(),
            target: "/photos".into(),
            read_only: true,
        });
        assert!(project_plan(&plan, "C:/apps/memos", &[]).is_err());
        plan.services[0].mounts.pop();
        for path in ["../outside", "data/../outside", "C:/outside", "data/a\nb"] {
            assert!(project_plan(
                &plan,
                "C:/apps/memos",
                &[SeedFile {
                    path: path.into(),
                    content: String::new()
                }]
            )
            .is_err());
        }
        assert!(project_plan(&plan, r"\\server\apps", &[]).is_err());
    }
}
