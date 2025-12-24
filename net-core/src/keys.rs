#[cfg(feature = "nostr-client")]
use crate::config::{KeyFormat, KeyPersistenceConfig};
#[cfg(feature = "nostr-client")]
use crate::error::{NetError, Result};
#[cfg(feature = "nostr-client")]
use radroots_nostr::prelude::{
    RadrootsNostrKeys,
    RadrootsNostrSecretKey,
    RadrootsNostrSecp256k1SecretKey,
    RadrootsNostrToBech32,
};
#[cfg(feature = "nostr-client")]
use serde::Deserialize;
#[cfg(feature = "nostr-client")]
use std::path::{Path, PathBuf};
#[cfg(feature = "nostr-client")]
use std::str::FromStr;

#[cfg(feature = "nostr-client")]
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct KeysFile {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[cfg(feature = "nostr-client")]
#[derive(Debug, Clone, Default)]
pub struct KeysState {
    pub loaded: bool,
    pub source: Option<PathBuf>,
    pub npub: Option<String>,
    pub last_error: Option<NetError>,
}

#[cfg(feature = "nostr-client")]
#[derive(Debug, Clone, Default)]
pub struct KeysManager {
    pub keys: Option<RadrootsNostrKeys>,
    pub state: KeysState,
}

#[cfg(feature = "nostr-client")]
#[derive(Debug, Clone)]
pub enum LoadOutcome {
    FromFile(PathBuf),
    GeneratedEphemeral,
}

#[cfg(feature = "nostr-client")]
impl KeysManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_valid_hex32(s: &str) -> bool {
        let is_hex = s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        is_hex
    }
    pub fn is_valid_nsec(s: &str) -> bool {
        s.starts_with("nsec1")
    }

    pub fn load_from_secret_bytes(&mut self, sk: &[u8; 32]) -> Result<()> {
        let secp =
            RadrootsNostrSecp256k1SecretKey::from_slice(&sk[..]).map_err(|_| NetError::InvalidHex32)?;
        let nostr_sk = RadrootsNostrSecretKey::from(secp);
        let keys = RadrootsNostrKeys::new(nostr_sk);
        self.set_keys(keys);
        Ok(())
    }

    pub fn load_from_hex32(&mut self, hex: &str) -> Result<()> {
        use secrecy::{ExposeSecret, SecretString};
        let secret = SecretString::new(hex.to_owned().into());
        let k = RadrootsNostrSecretKey::from_str(secret.expose_secret())
            .map_err(|_| NetError::InvalidHex32)?;
        let keys = RadrootsNostrKeys::new(k);
        self.set_keys(keys);
        Ok(())
    }

    pub fn load_from_nsec(&mut self, nsec: &str) -> Result<()> {
        use secrecy::{ExposeSecret, SecretString};
        let secret = SecretString::new(nsec.to_owned().into());
        let keys =
            RadrootsNostrKeys::parse(secret.expose_secret()).map_err(|_| NetError::InvalidBech32)?;
        self.set_keys(keys);
        Ok(())
    }

    pub fn set_keys(&mut self, keys: RadrootsNostrKeys) {
        let npub = keys.public_key().to_bech32().ok();
        self.keys = Some(keys);
        self.state.loaded = true;
        self.state.source = None;
        self.state.npub = npub;
        self.state.last_error = None;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn require(&self) -> Result<&RadrootsNostrKeys> {
        self.keys.as_ref().ok_or(NetError::MissingKey)
    }

    pub fn load_from_path_auto(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let p = path.as_ref();
        match std::fs::read_to_string(p) {
            Ok(s) => {
                if let Ok(jf) = serde_json::from_str::<KeysFile>(&s) {
                    return self.load_from_hex32(&jf.key).map(|_| {
                        self.state.source = Some(p.to_path_buf());
                    });
                }
                let trimmed = s.trim();
                if Self::is_valid_nsec(trimmed) {
                    return self.load_from_nsec(trimmed).map(|_| {
                        self.state.source = Some(p.to_path_buf());
                    });
                }
                if Self::is_valid_hex32(trimmed) {
                    return self.load_from_hex32(trimmed).map(|_| {
                        self.state.source = Some(p.to_path_buf());
                    });
                }
            }
            Err(_) => {}
        }
        match std::fs::read(p) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                self.load_from_secret_bytes(&arr)?;
                self.state.source = Some(p.to_path_buf());
                Ok(())
            }
            _ => Err(NetError::InvalidKeyFile),
        }
    }

    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.load_from_path_auto(path)
    }

    pub fn save_to_path_with_format(
        &self,
        path: impl AsRef<Path>,
        format: KeyFormat,
        no_overwrite: bool,
    ) -> Result<()> {
        if no_overwrite && path.as_ref().exists() {
            return Err(NetError::OverwriteDenied);
        }
        match format {
            KeyFormat::Json => self.save_json(path),
            KeyFormat::Nsec => self.save_nsec_text(path, no_overwrite),
            KeyFormat::Hex => self.save_hex_text(path, no_overwrite),
            KeyFormat::Bin => self.save_raw_bin(path, no_overwrite),
        }
    }

    fn require_secret_hex(&self) -> Result<String> {
        let keys = self.require()?;
        Ok(keys.secret_key().to_secret_hex())
    }

    pub fn export_secret_hex(&self) -> Result<String> {
        self.require_secret_hex()
    }

    fn save_json(&self, path: impl AsRef<Path>) -> Result<()> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let keys = self.require()?;
        let secret_hex = keys.secret_key().to_secret_hex();
        let payload = KeysFile {
            key: secret_hex,
            npub: self.npub(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs()),
            note: None,
        };
        let json = serde_json::to_string_pretty(&payload).map_err(|_| NetError::KeyIo)?;
        write_secret_atomically_noclobber(path.as_ref(), json.as_bytes())
            .map_err(|_| NetError::KeyIo)?;
        Ok(())
    }

    fn save_nsec_text(&self, path: impl AsRef<Path>, no_overwrite: bool) -> Result<()> {
        if no_overwrite && path.as_ref().exists() {
            return Err(NetError::OverwriteDenied);
        }
        let keys = self.require()?;
        let nsec = keys.secret_key().to_bech32().map_err(|_| NetError::KeyIo)?;
        write_secret_atomically_noclobber(path.as_ref(), nsec.as_bytes())
            .map_err(|_| NetError::KeyIo)?;
        Ok(())
    }

    fn save_hex_text(&self, path: impl AsRef<Path>, no_overwrite: bool) -> Result<()> {
        if no_overwrite && path.as_ref().exists() {
            return Err(NetError::OverwriteDenied);
        }
        let hex = self.require_secret_hex()?;
        write_secret_atomically_noclobber(path.as_ref(), hex.as_bytes())
            .map_err(|_| NetError::KeyIo)?;
        Ok(())
    }

    fn save_raw_bin(&self, path: impl AsRef<Path>, no_overwrite: bool) -> Result<()> {
        if no_overwrite && path.as_ref().exists() {
            return Err(NetError::OverwriteDenied);
        }
        let hex = self.require_secret_hex()?;
        let mut out = [0u8; 32];
        hex::decode_to_slice(hex, &mut out).map_err(|_| NetError::KeyIo)?;
        write_secret_atomically_noclobber(path.as_ref(), &out).map_err(|_| NetError::KeyIo)?;
        Ok(())
    }

    pub fn generate_in_memory(&mut self) -> &RadrootsNostrKeys {
        let keys = RadrootsNostrKeys::generate();
        self.set_keys(keys);
        self.keys.as_ref().unwrap()
    }

    pub fn ensure_loaded_from_file_outcome(
        &mut self,
        path: impl AsRef<Path>,
        allow_generate: bool,
    ) -> Result<LoadOutcome> {
        let p = path.as_ref();
        if p.exists() {
            self.load_from_path_auto(p)?;
            return Ok(LoadOutcome::FromFile(p.to_path_buf()));
        }
        if !allow_generate {
            self.state.last_error = Some(NetError::MissingKey);
            return Err(NetError::MissingKey);
        }
        let _ = self.generate_in_memory();
        Ok(LoadOutcome::GeneratedEphemeral)
    }

    pub fn ensure_loaded_from_file(
        &mut self,
        path: impl AsRef<Path>,
        allow_generate: bool,
    ) -> Result<()> {
        let _ = self.ensure_loaded_from_file_outcome(path, allow_generate)?;
        Ok(())
    }

    pub fn npub(&self) -> Option<String> {
        self.state.npub.clone()
    }

    #[cfg(all(feature = "directories", feature = "fs-persistence"))]
    pub fn default_key_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "Radroots", "radroots")
            .map(|d| d.config_dir().join("identity.json"))
    }

    #[cfg(all(feature = "directories", feature = "fs-persistence"))]
    pub fn persist_best_practice(&self) -> Result<PathBuf> {
        let path = Self::default_key_path().ok_or(NetError::PersistenceUnsupported)?;
        if path.exists() {
            return Err(NetError::OverwriteDenied);
        }
        self.save_to_path_with_format(&path, KeyFormat::Json, true)?;
        Ok(path)
    }

    #[cfg(not(all(feature = "directories", feature = "fs-persistence")))]
    pub fn persist_best_practice(&self) -> Result<PathBuf> {
        Err(NetError::PersistenceUnsupported)
    }

    #[cfg(feature = "fs-persistence")]
    pub fn persist_with_config(&self, cfg: &KeyPersistenceConfig) -> Result<PathBuf> {
        let path = if let Some(p) = &cfg.path {
            p.clone()
        } else {
            #[cfg(all(feature = "directories", feature = "fs-persistence"))]
            {
                Self::default_key_path().ok_or(NetError::PersistenceUnsupported)?
            }
            #[cfg(not(all(feature = "directories", feature = "fs-persistence")))]
            {
                return Err(NetError::PersistenceUnsupported);
            }
        };
        self.save_to_path_with_format(&path, cfg.format, cfg.no_overwrite)?;
        Ok(path)
    }

    #[cfg(not(feature = "fs-persistence"))]
    pub fn persist_with_config(&self, _cfg: &KeyPersistenceConfig) -> Result<PathBuf> {
        Err(NetError::PersistenceUnsupported)
    }
}

#[cfg(feature = "nostr-client")]
fn write_secret_atomically_noclobber(path: &Path, data: &[u8]) -> crate::error::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(data)?;
    tmp.flush()?;

    let persist_result = tmp.persist_noclobber(path);

    if let Err(e) = persist_result {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(crate::error::NetError::OverwriteDenied);
        } else {
            return Err(crate::error::NetError::KeyIo);
        }
    }

    #[cfg(unix)]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, Permissions::from_mode(0o600));
    }

    Ok(())
}
