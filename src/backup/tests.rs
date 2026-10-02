use super::*;

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "local-store-backup-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn fixture() -> Snapshot {
    Snapshot {
        schema_version: 1,
        contract: StorageContract {
            offering_id: "memos".into(),
            reviewed_plan_sha256: "a".repeat(64),
            bind_directories: vec!["data".into()],
            named_volumes: vec![],
            has_shared_host_folders: false,
        },
        engine: engine::EngineBinding::managed_wsl(),
        daemon_id_sha256: "b".repeat(64),
        files: vec![
            Entry {
                path: "data".into(),
                directory: true,
                sha256: hash(&[]),
                bytes: vec![],
                length: 0,
            },
            Entry {
                path: "data/private-note.db".into(),
                directory: false,
                sha256: hash(b"private exact content"),
                bytes: b"private exact content".to_vec(),
                length: 21,
            },
            Entry {
                path: runtime::SECRETS_FILE.into(),
                directory: false,
                sha256: hash(b"generated-private-secret"),
                bytes: b"generated-private-secret".to_vec(),
                length: 24,
            },
        ],
        volumes: vec![],
    }
}

#[test]
fn binary_framing_preserves_exact_private_bytes_and_rejects_truncation() {
    let snapshot = fixture();
    let encoded = encode(&snapshot).unwrap();
    let decoded = decode_plain(&encoded).unwrap();
    validate_entries(&decoded).unwrap();
    assert_eq!(decoded.files[1].bytes, b"private exact content");
    assert_eq!(decoded.files[2].bytes, b"generated-private-secret");
    assert!(decode_plain(&encoded[..encoded.len() - 1]).is_err());
    let mut appended = encoded.clone();
    appended.push(0);
    assert!(decode_plain(&appended).is_err());
    let mut bad_header = encoded;
    bad_header[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_plain(&bad_header).is_err());
}

#[cfg(windows)]
#[test]
fn dpapi_round_trip_is_private_and_tampering_is_refused() {
    let encoded = encode(&fixture()).unwrap();
    let encrypted = protection::protect(&encoded).unwrap();
    assert!(!encrypted
        .windows(24)
        .any(|part| part == b"generated-private-secret"));
    assert!(!encrypted
        .windows(21)
        .any(|part| part == b"private exact content"));
    let mut protected = MAGIC.to_vec();
    protected.extend(&encrypted);
    let restored = decode(&protected).unwrap();
    assert_eq!(restored.files[2].bytes, b"generated-private-secret");
    let last = protected.len() - 1;
    protected[last] ^= 0x40;
    assert!(decode(&protected).is_err());
}

#[cfg(windows)]
#[test]
fn dpapi_chunks_are_bound_to_their_archive_and_position() {
    let bytes = vec![0x5a; 130000];
    let encrypted = protection::protect(&bytes).unwrap();
    assert_eq!(protection::unprotect(&encrypted).unwrap(), bytes);
    let first = u32::from_le_bytes(encrypted[28..32].try_into().unwrap()) as usize;
    let second_offset = 32 + first;
    let second = u32::from_le_bytes(
        encrypted[second_offset..second_offset + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut swapped = encrypted[..28].to_vec();
    swapped.extend(&encrypted[second_offset..second_offset + 4 + second]);
    swapped.extend(&encrypted[28..second_offset]);
    swapped.extend(&encrypted[second_offset + 4 + second..]);
    assert!(protection::unprotect(&swapped).is_err());
    let mut nonce = encrypted.clone();
    nonce[12] ^= 1;
    assert!(protection::unprotect(&nonce).is_err());
    assert!(protection::unprotect(&encrypted[..encrypted.len() - 1]).is_err());
}

#[test]
fn restore_only_writes_fresh_data_and_preserves_unrelated_files() {
    let root = scratch("fresh");
    fs::write(root.join("compose.yaml"), b"reviewed compose").unwrap();
    restore_files(&root, &fixture()).unwrap();
    assert_eq!(
        fs::read(root.join("data/private-note.db")).unwrap(),
        b"private exact content"
    );
    assert_eq!(
        fs::read(root.join(runtime::SECRETS_FILE)).unwrap(),
        b"generated-private-secret"
    );
    assert_eq!(
        fs::read(root.join("compose.yaml")).unwrap(),
        b"reviewed compose"
    );
    assert!(restore_files(&root, &fixture()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn occupied_data_or_credentials_refuse_before_any_restore_write() {
    for existing in ["data/private-note.db", runtime::SECRETS_FILE] {
        let root = scratch("occupied");
        fs::create_dir(root.join("data")).unwrap();
        fs::write(root.join(existing), b"person's existing data").unwrap();
        assert!(restore_files(&root, &fixture()).is_err());
        assert_eq!(
            fs::read(root.join(existing)).unwrap(),
            b"person's existing data"
        );
        if existing != runtime::SECRETS_FILE {
            assert!(!root.join(runtime::SECRETS_FILE).exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn traversal_duplicate_cross_app_corruption_and_missing_data_are_refused() {
    for path in [
        "../other-app/secret",
        "/root/secret",
        "C:/private",
        "data/../private",
        "other-app/secret",
        "data/back\\slash",
        "data/trailing.",
    ] {
        let mut snapshot = fixture();
        snapshot.files[1].path = path.into();
        assert!(validate_entries(&snapshot).is_err(), "{path}");
    }
    let mut snapshot = fixture();
    snapshot.files[1].bytes.push(0);
    assert!(validate_entries(&snapshot).is_err());
    let mut snapshot = fixture();
    snapshot.files[2].path = snapshot.files[1].path.clone();
    assert!(validate_entries(&snapshot).is_err());
    let mut snapshot = fixture();
    snapshot.files.remove(0);
    assert!(validate_entries(&snapshot).is_err());
}

#[test]
fn named_volume_contract_is_explicit_and_missing_archive_is_refused() {
    let offering = crate::offerings::offering("gitea").unwrap();
    let contract = StorageContract::from_plan(&offering.plan_template(None).unwrap().plan).unwrap();
    assert_eq!(contract.named_volumes, ["gitea-data", "gitea-postgres"]);
    contract.supported().unwrap();
    let mut snapshot = fixture();
    snapshot.contract = contract;
    snapshot.files.clear();
    assert!(validate_entries(&snapshot).is_err());
    let mut shared = fixture().contract;
    shared.has_shared_host_folders = true;
    assert!(shared.supported().is_err());
}

fn tar_header(path: &str, kind: u8, mode: &str) -> Vec<u8> {
    let mut bytes = vec![0; 1536];
    bytes[..path.len()].copy_from_slice(path.as_bytes());
    bytes[100..108].copy_from_slice(mode.as_bytes());
    bytes[124..136].copy_from_slice(b"00000000000\0");
    bytes[156] = kind;
    bytes[257..263].copy_from_slice(b"ustar\0");
    bytes[148..156].fill(b' ');
    let checksum: usize = bytes[..512].iter().map(|byte| *byte as usize).sum();
    bytes[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
    bytes
}

#[test]
fn bounded_tar_parser_rejects_paths_links_devices_extensions_and_elevated_modes() {
    for (path, kind, mode) in [
        ("../escape", b'0', "0000644\0"),
        ("/absolute", b'0', "0000644\0"),
        ("data/link", b'2', "0000644\0"),
        ("data/hard", b'1', "0000644\0"),
        ("device", b'3', "0000644\0"),
        ("pax", b'x', "0000644\0"),
        ("elevated", b'0', "0004644\0"),
    ] {
        assert!(
            volumes::validate_tar(&tar_header(path, kind, mode)).is_err(),
            "{path}"
        );
    }
    volumes::validate_tar(&tar_header("./data/plain-file", b'0', "0000644\0")).unwrap();
    volumes::validate_tar(&tar_header("./", b'5', "0000755\0")).unwrap();
    let mut corrupt = tar_header("data/plain", b'0', "0000644\0");
    corrupt[0] ^= 1;
    assert!(volumes::validate_tar(&corrupt).is_err());
    let valid = tar_header("data/plain", b'0', "0000644\0");
    assert!(volumes::validate_tar(&valid[..1024]).is_err());
}

#[cfg(windows)]
#[test]
fn windows_reparse_points_are_refused_without_reading_link_target() {
    use std::os::windows::fs::symlink_dir;
    let root = scratch("reparse");
    let outside = scratch("outside");
    fs::write(outside.join("private"), b"another app secret").unwrap();
    // Symlink creation can require Windows developer mode; junction creation
    // is deliberately not smuggled into this unit test through a shell.
    if symlink_dir(&outside, root.join("data")).is_ok() {
        assert!(collect(&root, "data", &mut vec![], &mut 0, 0).is_err());
        fs::remove_dir(root.join("data")).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}
