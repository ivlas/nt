use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NtError::Message(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;
    let tmp_path = parent.join(tmp_name(path));

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)?;
    if let Err(err) = file.write_all(bytes) {
        drop(file);
        let _ = fs::remove_file(&tmp_path);
        return Err(err.into());
    }
    if let Err(err) = file.sync_all() {
        drop(file);
        let _ = fs::remove_file(&tmp_path);
        return Err(err.into());
    }
    drop(file);

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err.into());
    }

    Ok(())
}

fn tmp_name(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("nt.tmp");

    format!(".{file_name}.{}.{}.tmp", std::process::id(), tmp_nonce())
}

fn tmp_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::atomic_write;

    #[test]
    fn atomic_write_replaces_file_contents() {
        let dir = std::env::temp_dir().join(format!("nt-test-{}", std::process::id()));
        let path = dir.join("note.md");

        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");

        let _ = fs::remove_dir_all(dir);
    }
}
