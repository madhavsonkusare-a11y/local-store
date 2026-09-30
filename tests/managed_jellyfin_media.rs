//! Opt-in managed-engine proof of a Jellyfin library and sample playback.
#[cfg(windows)]
#[test]
#[ignore = "real managed WSL qualification; set LOCAL_STORE_RUN_MANAGED_QUALIFICATION=1"]
fn jellyfin_sample_media_survives_reinstall() {
    use local_store::{
        qualification::{qualify_on_engine_at, FirstUse, ScriptProbe},
        runtime::engine::EngineBinding,
    };
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    assert_eq!(
        std::env::var("LOCAL_STORE_RUN_MANAGED_QUALIFICATION").as_deref(),
        Ok("1")
    );
    const TASK: &str = "an administrator imports an owned sample audio file, streams its exact bytes, and repeats playback after restart and keep-data reinstall";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let media = root.join(format!(
        ".cache/managed-qualification/jellyfin-media-{nonce}"
    ));
    std::fs::create_dir_all(&media).expect("owned media folder created");
    let filename = "Local Store Proof Tone.wav";
    let media_file = media.join(filename);

    // A one-second 8 kHz mono PCM tone is a deterministic, locally owned fixture.
    let samples: Vec<i16> = (0..8000)
        .map(|index| {
            let radians = index as f64 * std::f64::consts::TAU * 440.0 / 8000.0;
            (radians.sin() * 8000.0) as i16
        })
        .collect();
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8000u32.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(&media_file, &wav).expect("owned audio fixture written");

    let state = root.join(format!(
        ".cache/managed-qualification/jellyfin-media-{nonce}.json"
    ));
    let probe =
        ScriptProbe::new(root.join("scripts/jellyfin-media-probe.mjs"), TASK).with_args(vec![
            state.to_string_lossy().into_owned(),
            media_file.to_string_lossy().into_owned(),
        ]);
    let fields = BTreeMap::from([
        (
            "FOLDER_MEDIA".to_owned(),
            media.to_string_lossy().into_owned(),
        ),
        ("TZ".to_owned(), "Asia/Kolkata".to_owned()),
    ]);
    let result = qualify_on_engine_at(
        "jellyfin",
        &fields,
        &probe as &dyn FirstUse,
        &root.join(".cache/managed-qualification"),
        &EngineBinding::managed_wsl(),
        &root.join(".cache/engine/real-wsl-proof/state"),
    );
    let _ = std::fs::remove_file(&state);
    let _ = std::fs::remove_file(&media_file);
    let _ = std::fs::remove_dir(&media);
    let evidence = result.expect("managed qualification could run");
    let output = root.join("docs/evidence/jellyfin-managed-media-2026-09-30.json");
    std::fs::write(&output, format!("{}\n", evidence.to_json())).expect("evidence written");
    assert!(
        evidence.passed,
        "Jellyfin media failed: {:?}",
        evidence.failure()
    );
    assert!(evidence.measurements.is_some());
}
