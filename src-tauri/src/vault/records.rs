use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::{de::DeserializeOwned, Serialize};
use std::io::{self, Write};
use std::path::{Component, Path};
use uuid::Uuid;

pub(crate) fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Vault record path must be non-empty, relative, and traversal-free".to_owned());
    }
    Ok(())
}

pub(crate) fn write_new_json(
    directory: &Dir,
    relative: &Path,
    value: &impl Serialize,
) -> Result<(), String> {
    validate_relative_path(relative)?;
    match directory.symlink_metadata(relative) {
        Ok(_) => return Err("Vault record already exists".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Vault record path cannot be inspected: {error}")),
    }

    let parent = relative
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "Vault record must have a parent directory".to_owned())?;
    let file_name = relative
        .file_name()
        .ok_or_else(|| "Vault record filename is missing".to_owned())?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4().simple()));

    let result = (|| -> Result<(), String> {
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut file = directory
            .open_with(&temporary, &options)
            .map_err(|error| format!("Vault record temporary file cannot be created: {error}"))?;
        let bytes = serde_json::to_vec(value)
            .map_err(|error| format!("Vault record cannot be serialized: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("Vault record cannot be written: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Vault record cannot be synchronized: {error}"))?;

        match directory.symlink_metadata(relative) {
            Ok(_) => return Err("Vault record appeared before atomic publish".to_owned()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Vault record final path cannot be inspected: {error}"
                ))
            }
        }
        directory
            .rename(&temporary, directory, relative)
            .map_err(|error| format!("Vault record cannot be published atomically: {error}"))
    })();

    if result.is_err() {
        let _ = directory.remove_file(&temporary);
    }
    result
}

pub(crate) fn read_json<T: DeserializeOwned>(
    directory: &Dir,
    relative: &Path,
) -> Result<T, String> {
    validate_relative_path(relative)?;
    let file = directory
        .open(relative)
        .map_err(|error| format!("Vault record cannot be opened: {error}"))?;
    serde_json::from_reader(file).map_err(|error| format!("Vault record is invalid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{read_json, validate_relative_path, write_new_json};
    use cap_std::ambient_authority;
    use cap_std::fs::Dir;
    use serde::Serialize;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug, PartialEq, Eq, serde::Deserialize, Serialize)]
    struct Record {
        value: String,
    }

    fn temporary_directory() -> PathBuf {
        let unique = format!(
            "aiknowledgesort-records-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir(&path).expect("create generated record directory");
        path.canonicalize().expect("canonical record directory")
    }

    #[test]
    fn rejects_absolute_parent_and_empty_record_paths() {
        for path in [
            Path::new(""),
            Path::new("../escape"),
            Path::new("/absolute"),
        ] {
            assert!(validate_relative_path(path).is_err());
        }
        assert!(validate_relative_path(Path::new(".aiks/operations/one.json")).is_ok());
    }

    #[test]
    fn writes_a_new_json_record_without_replacing_it() {
        let root = temporary_directory();
        fs::create_dir_all(root.join(".aiks/operations")).expect("create record parent");
        let directory =
            Dir::open_ambient_dir(&root, ambient_authority()).expect("open generated root");
        let relative = Path::new(".aiks/operations/one.json");

        write_new_json(
            &directory,
            relative,
            &Record {
                value: "first".to_owned(),
            },
        )
        .expect("write first record");
        assert!(write_new_json(
            &directory,
            relative,
            &Record {
                value: "second".to_owned(),
            },
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(root.join(relative)).expect("read record"),
            "{\"value\":\"first\"}"
        );
        assert_eq!(
            read_json::<Record>(&directory, relative).expect("read JSON record"),
            Record {
                value: "first".to_owned()
            }
        );

        fs::remove_dir_all(root).expect("remove generated record directory");
    }
}
