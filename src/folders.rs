//! Folders on this computer that a person chooses to share with an app.
use std::path::{Component, Path, PathBuf};

/// A canonical path in the form Docker can actually use.
///
/// Canonicalizing is what stops a path that merely *points* somewhere refused,
/// and on Windows it produces an extended-length path (`\?\C:\...`). Docker
/// builds a mount source out of that and refuses the whole thing with "too
/// many colons". Stripping the prefix keeps the resolved target — the same
/// directory, named the way the rest of the system names it.
pub fn docker_path(path: &Path) -> PathBuf {
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => path.to_path_buf(),
    }
}

/// A folder on this computer that a person has chosen to share with an app.
///
/// This is the one place the product hands a container something outside the
/// storage it manages, so it is the one place worth being slow about. The apps
/// that need it are the ones people most want — a photo library, a music
/// collection, a shelf of books — and none of those live inside an
/// application's own data directory. Refusing them entirely was costing
/// forty-one of the hundred and fifty most popular open-source apps.
///
/// What makes it safe is not that the path is checked once, but what the check
/// refuses: anything that would let an app read the operating system, the
/// person's whole profile, or Local Store's own records of every other app.
/// A mount is a capability, and this decides how far it reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedFolder {
    /// The resolved absolute path, with symlinks followed.
    pub path: PathBuf,
    /// Whether the app may write to it, or only read.
    pub read_only: bool,
}

/// Why a folder cannot be shared, in words a person can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRefusal(pub String);

impl std::fmt::Display for FolderRefusal {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

/// Directory names that are the operating system, not a person's files.
///
/// Compared against the resolved path's leading components rather than by
/// prefix string matching, so `C:\WindowsApps-of-mine` is not caught by
/// `C:\Windows` and `/usrlocal` is not caught by `/usr`.
#[cfg(windows)]
const SYSTEM_ROOTS: &[&str] = &[
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "$Recycle.Bin",
    "System Volume Information",
    "Recovery",
    "PerfLogs",
];

#[cfg(not(windows))]
const SYSTEM_ROOTS: &[&str] = &[
    "etc", "usr", "bin", "sbin", "lib", "lib64", "boot", "dev", "proc", "sys", "var", "run",
];

/// Decide whether an app may be given this folder.
///
/// `managed_root` is Local Store's own configuration directory, which is
/// refused along with everything inside it: an app handed that folder could
/// read the registry of every installed app and the credentials generated for
/// them.
pub fn share_folder(
    answer: &str,
    read_only: bool,
    managed_root: &Path,
) -> Result<SharedFolder, FolderRefusal> {
    let refuse = |reason: &str| Err(FolderRefusal(reason.to_owned()));
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return refuse("Choose a folder to share with this app.");
    }
    // Control characters and quotes are how a path turns into something else
    // by the time Compose has read it.
    if trimmed
        .chars()
        .any(|c| c.is_control() || c == '"' || c == '\'')
    {
        return refuse("That folder name contains characters that are not allowed.");
    }
    let raw = Path::new(trimmed);
    if !raw.is_absolute() {
        return refuse("Enter the full path to the folder, starting from the drive or root.");
    }

    // Resolving follows symlinks, which is what stops a shared folder that
    // merely *points* at somewhere refused. Everything below judges the
    // resolved path, never what was typed.
    let path = match raw.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return refuse("That folder does not exist. Create it first, then choose it.");
        }
    };
    if !path.is_dir() {
        return refuse("That is a file. Choose a folder instead.");
    }

    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if components.is_empty() {
        return refuse("Choose a folder inside a drive, not the whole drive.");
    }
    if SYSTEM_ROOTS
        .iter()
        .any(|system| components[0].eq_ignore_ascii_case(system))
    {
        return refuse("That folder belongs to the operating system and cannot be shared.");
    }

    // A home directory itself is everything a person has, including their
    // credentials. A folder inside it is an ordinary choice.
    if let Some(home) = home_directory() {
        if let Ok(home) = home.canonicalize() {
            if path == home {
                return refuse(
                    "That is your whole home folder. Choose a folder inside it instead.",
                );
            }
        }
    }
    // Local Store's own directory holds the registry of every installed app
    // and the credentials generated for them. An app given this would be given
    // all of that.
    if let Ok(managed) = managed_root.canonicalize() {
        if path == managed || path.starts_with(&managed) || managed.starts_with(&path) {
            return refuse(
                "That folder holds Local Store's own records. Choose one of your own folders.",
            );
        }
    }

    Ok(SharedFolder {
        // Handed on in the form Docker can mount, not the extended-length form
        // canonicalizing produced.
        path: docker_path(&path),
        read_only,
    })
}

fn home_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "local-store-folder-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn managed() -> PathBuf {
        std::env::temp_dir().join("local-store-managed-that-does-not-exist")
    }

    #[test]
    fn a_folder_a_person_owns_is_shared_with_its_resolved_path() {
        let dir = scratch("ok");
        let shared = share_folder(dir.to_str().unwrap(), false, &managed()).expect("allowed");
        // The same directory, in the form Docker can mount: comparing against
        // the raw canonical path would compare against a form that cannot be
        // used, which is what `docker_path` exists to avoid.
        assert_eq!(
            shared.path.canonicalize().unwrap(),
            dir.canonicalize().unwrap()
        );
        assert!(!shared.read_only);
        // Read-only travels with the decision rather than being reapplied later.
        let locked = share_folder(dir.to_str().unwrap(), true, &managed()).unwrap();
        assert!(locked.read_only);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The lesson adoption already taught: a canonicalized Windows path
    /// carries an extended-length prefix, and Docker refuses a mount source
    /// built from it with "too many colons". A shared folder is a mount
    /// source, so it must never leave here in that form.
    #[test]
    fn a_shared_folder_is_handed_on_in_a_form_docker_can_mount() {
        let dir = scratch("prefix");
        let shared = share_folder(dir.to_str().unwrap(), false, &managed()).expect("allowed");
        let text = shared.path.to_string_lossy();
        assert!(!text.starts_with(r"\?\"), "{text}");
        // Still the same directory, not a different one.
        assert!(shared.path.is_dir());
        assert_eq!(
            shared.path.canonicalize().unwrap(),
            dir.canonicalize().unwrap()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A home folder is everything a person has. The message has to say that,
    /// rather than talking about Local Store's records, which merely happen to
    /// live inside it.
    #[test]
    fn a_whole_home_folder_is_refused_in_its_own_words() {
        let Some(home) = home_directory() else {
            return;
        };
        // Some sandboxed runners can stat a profile directory while denying
        // canonicalization. The share path cannot pass its first check there.
        if home.canonicalize().is_err() {
            return;
        }
        let error = share_folder(home.to_str().unwrap(), false, &managed())
            .expect_err("a home folder must be refused");
        assert!(error.0.contains("home folder"), "{error}");
    }

    #[test]
    fn a_relative_or_empty_answer_is_refused_before_anything_touches_the_disk() {
        for answer in ["", "   ", "media", "./media", "..", "~/Music"] {
            assert!(
                share_folder(answer, false, &managed()).is_err(),
                "{answer:?} was accepted"
            );
        }
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused_rather_than_created() {
        let missing = scratch("missing").join("not-here");
        let error = share_folder(missing.to_str().unwrap(), false, &managed()).unwrap_err();
        assert!(error.0.contains("does not exist"), "{error}");
        assert!(!missing.exists(), "the check created the folder");
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let dir = scratch("file");
        let file = dir.join("note.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(share_folder(file.to_str().unwrap(), false, &managed()).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The operating system is not a person's files, and an app that could
    /// read it could read everything.
    #[test]
    fn the_operating_system_is_never_shared() {
        let candidates: &[&str] = if cfg!(windows) {
            &[
                r"C:\Windows",
                r"C:\Windows\System32",
                r"C:\Program Files",
                r"C:\ProgramData",
            ]
        } else {
            &["/etc", "/usr", "/var", "/proc", "/etc/ssl"]
        };
        for path in candidates {
            if !Path::new(path).exists() {
                continue;
            }
            let error =
                share_folder(path, false, &managed()).expect_err(&format!("{path} was allowed"));
            assert!(error.0.contains("operating system"), "{path}: {error}");
        }
    }

    /// A name that merely starts with a system name is somebody's own folder.
    #[test]
    fn a_folder_that_only_looks_like_a_system_one_is_allowed() {
        let dir = scratch("Windows-backup");
        assert!(share_folder(dir.to_str().unwrap(), false, &managed()).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Local Store's own directory holds the registry of every installed app
    /// and the credentials generated for them.
    #[test]
    fn local_stores_own_records_are_never_shared() {
        let root = scratch("managed");
        let inside = root.join("apps");
        std::fs::create_dir_all(&inside).unwrap();
        for path in [&root, &inside] {
            let error = share_folder(path.to_str().unwrap(), false, &root)
                .expect_err("the managed root must be refused");
            assert!(error.0.contains("Local Store"), "{error}");
        }
        // And a parent of it, which would contain it.
        let parent = root.parent().unwrap();
        assert!(share_folder(parent.to_str().unwrap(), false, &root).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    /// Following symlinks is the point: a folder that merely *points* at a
    /// refused place must not get through by being named something else.
    #[cfg(unix)]
    #[test]
    fn a_link_to_a_refused_place_is_judged_by_where_it_leads() {
        let dir = scratch("link");
        let link = dir.join("innocent");
        std::os::unix::fs::symlink("/etc", &link).unwrap();
        let error = share_folder(link.to_str().unwrap(), false, &managed())
            .expect_err("a link to /etc must be refused");
        assert!(error.0.contains("operating system"), "{error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_path_with_characters_that_could_change_its_meaning_is_refused() {
        for answer in ["/tmp/a\"b", "/tmp/a'b", "/tmp/a\nb"] {
            assert!(
                share_folder(answer, false, &managed()).is_err(),
                "{answer:?} was accepted"
            );
        }
    }
}
