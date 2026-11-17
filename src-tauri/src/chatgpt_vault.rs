//! Windows credential storage and legacy OS-vault compatibility.
//! macOS login now uses private, app-owned records; never touches old Keychain entries.

pub trait Vault: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn remove(&self, account: &str) -> Result<(), String>;
}
#[cfg(any(not(unix), test))]
pub use system::SystemVault;

#[cfg(any(not(unix), test))]
mod system {
    use super::Vault;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Serialize};
    pub struct SystemVault;
    const SERVICE: &str = "com.fishyj.liubai.chatgpt";
    const ERROR: &str = "无法访问系统凭据库，请解锁钥匙串或检查凭据访问权限";
    // Windows generic credentials have a small blob limit. An atomic manifest points
    // to a complete generation of short entries; interrupted writes retain the old one.
    const PART: usize = 1800;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        generation: String,
        count: usize,
    }
    fn entry(name: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, name).map_err(|_| ERROR.into())
    }
    fn read(name: &str) -> Result<Option<String>, String> {
        match entry(name)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(ERROR.into()),
        }
    }
    fn erase(name: &str) -> Result<(), String> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(ERROR.into()),
        }
    }
    fn manifest(raw: &str) -> Result<Manifest, String> {
        let m: Manifest = serde_json::from_str(raw).map_err(|_| ERROR)?;
        if uuid::Uuid::parse_str(&m.generation).is_err() || m.count == 0 || m.count > 64 {
            return Err(ERROR.into());
        }
        Ok(m)
    }
    fn remove_parts(account: &str, m: &Manifest) {
        for i in 0..m.count {
            let _ = erase(&format!("{account}:{}:{i}", m.generation));
        }
    }
    impl Vault for SystemVault {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            let Some(raw) = read(account)? else {
                return Ok(None);
            };
            let m = manifest(&raw)?;
            let mut encoded = String::new();
            for i in 0..m.count {
                let p = read(&format!("{account}:{}:{i}", m.generation))?.ok_or(ERROR)?;
                if p.len() > PART {
                    return Err(ERROR.into());
                }
                encoded.push_str(&p);
            }
            let bytes = STANDARD.decode(encoded).map_err(|_| ERROR)?;
            String::from_utf8(bytes).map(Some).map_err(|_| ERROR.into())
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            if secret.len() > 64 * 1024 {
                return Err(ERROR.into());
            }
            let previous = read(account)?.map(|v| manifest(&v)).transpose()?;
            let encoded = STANDARD.encode(secret);
            let m = Manifest {
                generation: uuid::Uuid::new_v4().to_string(),
                count: encoded.len().div_ceil(PART),
            };
            if m.count == 0 || m.count > 64 {
                return Err(ERROR.into());
            }
            let result = (|| {
                for (i, p) in encoded.as_bytes().chunks(PART).enumerate() {
                    entry(&format!("{account}:{}:{i}", m.generation))?
                        .set_password(std::str::from_utf8(p).map_err(|_| ERROR)?)
                        .map_err(|_| ERROR)?;
                }
                entry(account)?
                    .set_password(&serde_json::to_string(&m).map_err(|_| ERROR)?)
                    .map_err(|_| ERROR)?;
                Ok(())
            })();
            if result.is_err() {
                remove_parts(account, &m)
            } else if let Some(old) = previous {
                remove_parts(account, &old)
            }
            result
        }
        fn remove(&self, account: &str) -> Result<(), String> {
            if let Some(raw) = read(account)? {
                let m = manifest(&raw)?;
                // Keep the manifest on failure so cleanup can be retried. The auth
                // service disables the account before attempting vault cleanup.
                for i in 0..m.count {
                    erase(&format!("{account}:{}:{i}", m.generation))?;
                }
                erase(account)?;
            }
            Ok(())
        }
    }
}
