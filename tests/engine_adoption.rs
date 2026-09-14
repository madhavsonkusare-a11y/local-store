//! Opt-in proof of engine binding across a real install/restart/reinstall.
use local_store::{
    model::RuntimeSpec,
    qualification::{self, FirstUse, Phase},
    runtime::{
        engine::{self, EngineBinding},
        SystemProcessRunner,
    },
};
use std::path::PathBuf;

struct RestoreContext(Option<std::ffi::OsString>);
impl Drop for RestoreContext {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => std::env::set_var("DOCKER_CONTEXT", value),
            None => std::env::remove_var("DOCKER_CONTEXT"),
        }
    }
}

struct BindingProbe(EngineBinding);
impl FirstUse for BindingProbe {
    fn describes(&self) -> &str {
        "legacy engine adoption survives restart and reinstall with an invalid ambient Docker context"
    }
    fn exercise(&self, phase: Phase, _: &str) -> Result<(), String> {
        let registry = local_store::storage::load_registry_v2().map_err(|e| e.to_string())?;
        let app = registry
            .apps
            .first()
            .ok_or("isolated install has no registry entry")?;
        let RuntimeSpec::Compose { project_dir, .. } = &app.runtime else {
            return Err("not a managed app".into());
        };
        if engine::retained(project_dir).map_err(|e| e.message)? != Some(self.0.clone()) {
            return Err("installation lost or changed its engine binding".into());
        }
        if phase == Phase::FirstInstall {
            // Only qualification's private project: emulate an installation made
            // before bindings existed, preserving the original Compose bytes.
            let compose_path = project_dir.join("compose.yaml");
            let marked = std::fs::read_to_string(&compose_path).map_err(|e| e.to_string())?;
            let legacy = marked
                .strip_prefix(engine::COMPOSE_BINDING_MARKER)
                .ok_or("isolated fixture has no binding marker")?;
            std::fs::write(&compose_path, legacy).map_err(|e| e.to_string())?;
            std::fs::remove_file(project_dir.join(engine::ENGINE_FILE))
                .map_err(|e| e.to_string())?;
            let adopted = engine::adopt_current_engine(&app.id);
            // Restore the fixture on refusal so qualification can still clean up
            // through its captured endpoint; preserve the original error.
            if let Err(error) = adopted {
                engine::save(project_dir, &self.0).map_err(|e| e.message)?;
                std::fs::write(&compose_path, &marked).map_err(|e| e.to_string())?;
                return Err(error.message);
            }
            if adopted.map_err(|e| e.message)? != self.0
                || engine::retained(project_dir).map_err(|e| e.message)? != Some(self.0.clone())
                || std::fs::read_to_string(&compose_path).map_err(|e| e.to_string())? != marked
            {
                return Err("adoption changed the engine or Compose content".into());
            }
        }
        // Only this dedicated test process. Later lifecycle and qualification
        // commands must ignore this invalid ambient context, including cleanup.
        std::env::set_var("DOCKER_CONTEXT", "local-store-invalid-proof");
        Ok(())
    }
}

#[test]
#[ignore = "real Docker; set LOCAL_STORE_RUN_DOCKER_TEST=1"]
fn legacy_memos_adopts_its_engine_and_survives_context_changes() {
    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_DOCKER_TEST").as_deref(),
        Ok("1")
    );
    let binding = EngineBinding::discover(&SystemProcessRunner).unwrap();
    let _restore = RestoreContext(std::env::var_os("DOCKER_CONTEXT"));
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".cache");
    let evidence = qualification::qualify(
        "memos",
        &Default::default(),
        &BindingProbe(binding),
        &scratch,
    )
    .unwrap();
    std::fs::write(
        scratch.join("engine-adoption-memos-proof.json"),
        evidence.to_json(),
    )
    .unwrap();
    assert!(evidence.passed, "{:?}", evidence.failure());
}
