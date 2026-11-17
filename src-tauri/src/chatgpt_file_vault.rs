//! Protected, app-owned OAuth records. No OS permission dialog or external runtime.
//! Matches OpenAI's native-app guidance: atomic writes, private directory and 0600 files.
use crate::chatgpt_vault::Vault;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

const ERROR: &str = "无法读取本机登录授权，请点击重新连接 ChatGPT";
const WRITE_ERROR: &str = "无法安全保存登录授权，请检查应用数据目录权限后重试";
const LIMIT: u64 = 64 * 1024;

pub struct FileVault {
    directory: PathBuf,
}
impl FileVault {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn path(&self, account: &str) -> Result<PathBuf, String> {
        uuid::Uuid::parse_str(account).map_err(|_| ERROR)?;
        Ok(self.directory.join(format!("{account}.json")))
    }
    fn check_directory(&self, create: bool) -> Result<bool, String> {
        if create {
            // Do not follow a replaced app-owned directory when saving secrets.
            for path in [self.directory.parent().ok_or(WRITE_ERROR)?, &self.directory] {
                if let Ok(meta) = fs::symlink_metadata(path) {
                    if !meta.is_dir() || meta.file_type().is_symlink() {
                        return Err(WRITE_ERROR.into());
                    }
                }
            }
            fs::create_dir_all(&self.directory).map_err(|_| WRITE_ERROR)?;
        }
        let meta = match fs::symlink_metadata(&self.directory) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !create => return Ok(false),
            Err(_) => return Err(ERROR.into()),
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(ERROR.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if meta.uid() != unsafe { libc::geteuid() } {
                return Err(ERROR.into());
            }
            if create {
                fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))
                    .map_err(|_| WRITE_ERROR)?;
            } else if meta.mode() & 0o077 != 0 {
                return Err(ERROR.into());
            }
        }
        Ok(true)
    }
}
impl Vault for FileVault {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        let path = self.path(account)?;
        if !self.check_directory(false)? {
            return Ok(None);
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let file = match options.open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ERROR.into()),
        };
        let meta = file.metadata().map_err(|_| ERROR)?;
        if !meta.is_file() || meta.len() > LIMIT {
            return Err(ERROR.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
                return Err(ERROR.into());
            }
        }
        let mut bytes = Vec::new();
        file.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ERROR)?;
        if bytes.len() > LIMIT as usize {
            return Err(ERROR.into());
        }
        String::from_utf8(bytes).map(Some).map_err(|_| ERROR.into())
    }
    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        let path = self.path(account)?;
        if secret.is_empty() || secret.len() > LIMIT as usize {
            return Err(WRITE_ERROR.into());
        }
        self.check_directory(true)?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.directory).map_err(|_| WRITE_ERROR)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| WRITE_ERROR)?;
        }
        temp.write_all(secret.as_bytes()).map_err(|_| WRITE_ERROR)?;
        temp.as_file().sync_all().map_err(|_| WRITE_ERROR)?;
        temp.persist(path).map_err(|_| WRITE_ERROR)?;
        fs::File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| WRITE_ERROR)?;
        Ok(())
    }
    fn remove(&self, account: &str) -> Result<(), String> {
        let path = self.path(account)?;
        if !self.check_directory(false)? {
            return Ok(());
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(ERROR.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_atomic_records_survive_relaunch_and_remove_only_selected_account() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("credentials");
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        let vault = FileVault::new(path.clone());
        assert!(vault.get(&first).unwrap().is_none());
        vault.set(&first, "old synthetic token").unwrap();
        vault.set(&second, "another synthetic token").unwrap();
        vault.set(&first, "rotated synthetic token").unwrap();
        let reopened = FileVault::new(path.clone());
        assert_eq!(
            reopened.get(&first).unwrap().as_deref(),
            Some("rotated synthetic token")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(vault.path(&first).unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert!(vault.set("../escape", "test").is_err());
        assert!(vault.set(&first, &"x".repeat(LIMIT as usize + 1)).is_err());
        assert_eq!(fs::read_dir(&path).unwrap().count(), 2);
        reopened.remove(&first).unwrap();
        assert!(reopened.get(&first).unwrap().is_none());
        assert_eq!(
            reopened.get(&second).unwrap().as_deref(),
            Some("another synthetic token")
        );
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_and_publicly_readable_authorization() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("credentials");
        let id = uuid::Uuid::new_v4().to_string();
        let vault = FileVault::new(path.clone());
        vault.set(&id, "synthetic").unwrap();
        let file = vault.path(&id).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(vault.get(&id).is_err());
        fs::remove_file(&file).unwrap();
        let outside = root.path().join("outside");
        fs::write(&outside, "untouched").unwrap();
        symlink(&outside, &file).unwrap();
        assert!(vault.get(&id).is_err());
        vault.set(&id, "replacement").unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "untouched");
        assert!(!fs::symlink_metadata(&file)
            .unwrap()
            .file_type()
            .is_symlink());
        fs::rename(&path, root.path().join("original")).unwrap();
        symlink(root.path().join("original"), &path).unwrap();
        assert!(vault.set(&id, "no write").is_err());
    }
}
