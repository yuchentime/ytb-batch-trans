use base64::engine::general_purpose;
use base64::Engine;
use rand::Rng;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_keyring::KeyringExt;
use tauri_plugin_stronghold::stronghold::Stronghold;

pub const CLIENT: &[u8] = b"ovd";
const KR_SERVICE: &str = "com.jelleglebbeek.youtube-dl-gui";
const KR_ACCOUNT: &str = "master_key";

/// Stronghold key of the DeepSeek API key. The five existing auth keys keep their exact
/// names and semantics (M1); this one is only read by the translation client (AC-12).
pub const AI_API_KEY: &str = "ai.apiKey";

/// Wrapper for the DeepSeek API key that deliberately implements neither `Debug` nor `Display`
/// (nor `Serialize`), so the secret cannot leak into logs, IPC events or the config store by
/// accident (AC-12). Use [`ApiKey::expose`] only when building the `Authorization` header.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(Vec<u8>);

impl ApiKey {
  pub fn new(bytes: Vec<u8>) -> Self {
    Self(bytes)
  }

  /// Byte view for the request header. Never pass the result to a formatter.
  pub fn expose(&self) -> &[u8] {
    &self.0
  }
}

impl Drop for ApiKey {
  fn drop(&mut self) {
    self.0.fill(0);
  }
}

/// Trims a raw stronghold value and treats blank values as "not configured".
fn api_key_from_raw(raw: Option<Vec<u8>>) -> Result<Option<ApiKey>, String> {
  let Some(bytes) = raw else {
    return Ok(None);
  };
  let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
  let trimmed = text.trim();
  if trimmed.is_empty() {
    return Ok(None);
  }
  Ok(Some(ApiKey::new(trimmed.as_bytes().to_vec())))
}

#[derive(Debug, Default, Clone)]
pub struct AuthSecrets {
  pub username: Option<String>,
  pub password: Option<String>,
  pub video_password: Option<String>,
  pub bearer_token: Option<String>,
  pub headers: Vec<String>,
}

pub struct StrongholdState {
  pub snapshot_path: PathBuf,
  pub inner: Mutex<Option<Stronghold>>,
  pub init_error: Mutex<Option<String>>,
}

impl StrongholdState {
  pub const fn new(snapshot_path: PathBuf) -> Self {
    Self {
      snapshot_path,
      inner: Mutex::new(None),
      init_error: Mutex::new(None),
    }
  }
  pub fn load_auth_secrets(&self) -> Result<AuthSecrets, String> {
    let (username, password, video_password, bearer_token, headers) = {
      let guard = self
        .inner
        .lock()
        .map_err(|_| "failed to lock stronghold".to_string())?;
      let sh = guard.as_ref().ok_or_else(|| "vault locked".to_string())?;
      // The IPC commands write into the named `ovd` client; `sh.store()` is the default
      // client's store and would always look empty (M1 must keep working).
      let client = sh
        .get_client(CLIENT)
        .map_err(|e| format!("get_client failed: {e}"))?;
      let store = client.store();

      let get = |key: &str| -> Result<Option<String>, String> {
        match store.get(key.as_bytes()).map_err(|e| e.to_string())? {
          Some(bytes) if !bytes.is_empty() => {
            let s = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            let t = s.trim();
            if t.is_empty() {
              Ok(None)
            } else {
              Ok(Some(t.to_string()))
            }
          }
          _ => Ok(None),
        }
      };

      let username = get("auth.username")?;
      let password = get("auth.password")?;
      let video_password = get("video.password")?;
      let bearer_token = get("auth.bearer")?;

      let mut headers: Vec<String> = Vec::new();
      if let Some(hblob) = get("auth.headers")? {
        for line in hblob.lines() {
          let line = line.trim();
          if line.is_empty() || !line.contains(':') {
            continue;
          }
          headers.push(line.to_string());
        }
      }

      (username, password, video_password, bearer_token, headers)
    };

    Ok(AuthSecrets {
      username,
      password,
      video_password,
      bearer_token,
      headers,
    })
  }

  /// Reads the DeepSeek API key from the vault. Returns `Ok(None)` when unset/blank; the
  /// caller must not format the returned value (AC-12).
  pub fn load_ai_api_key(&self) -> Result<Option<ApiKey>, String> {
    let guard = self
      .inner
      .lock()
      .map_err(|_| "failed to lock stronghold".to_string())?;
    let sh = guard.as_ref().ok_or_else(|| "vault locked".to_string())?;
    let client = sh
      .get_client(CLIENT)
      .map_err(|e| format!("get_client failed: {e}"))?;
    let raw = client
      .store()
      .get(AI_API_KEY.as_bytes())
      .map_err(|e| e.to_string())?;
    api_key_from_raw(raw)
  }
}

fn generate_master_key() -> [u8; 32] {
  let mut key = [0u8; 32];
  rand::rng().fill_bytes(&mut key);
  key
}

fn open_with_key(state: &StrongholdState, key: &[u8]) -> Result<(), String> {
  let sh = Stronghold::new(&state.snapshot_path, key.to_vec())
    .map_err(|e| format!("stronghold open failed: {e}"))?;

  if sh.load_client(CLIENT).is_err() {
    sh.create_client(CLIENT)
      .map_err(|e| format!("create_client failed: {e}"))?;
    sh.write_client(CLIENT)
      .map_err(|e| format!("write_client failed: {e}"))?;
    sh.save().map_err(|e| format!("save failed: {e}"))?;
  }

  *state.inner.lock().unwrap() = Some(sh);
  Ok(())
}

fn create_new_stronghold(state: &StrongholdState, key: &[u8]) -> Result<(), String> {
  open_with_key(state, key)
}

pub fn init(app: &AppHandle, state: &State<StrongholdState>) -> Result<(), String> {
  create_and_store_new(app, state)
}

pub fn init_on_startup(app: &AppHandle, state: &State<StrongholdState>) {
  if !state.snapshot_path.exists() {
    return;
  }

  *state.init_error.lock().unwrap() = None;

  let keyring = app.keyring();
  match keyring.get_password(KR_SERVICE, KR_ACCOUNT) {
    Ok(opt) => {
      if let Some(b64) = opt {
        match general_purpose::STANDARD.decode(&b64) {
          Ok(key_bytes) => {
            if let Err(_e) = open_with_key(state, &key_bytes) {
              if let Err(setup_err) = create_and_store_new(app, state) {
                *state.init_error.lock().unwrap() =
                  Some(format!("Recreate vault failed after bad key: {setup_err}"));
              }
            }
          }
          Err(_) => {
            if let Err(setup_err) = create_and_store_new(app, state) {
              *state.init_error.lock().unwrap() = Some(format!(
                "Recreate vault failed (malformed key): {setup_err}"
              ));
            }
          }
        }
      }
    }
    Err(e) => {
      *state.init_error.lock().unwrap() = Some(format!("Secure keyring unavailable: {e}"));
    }
  }
}

fn create_and_store_new(app: &AppHandle, state: &State<StrongholdState>) -> Result<(), String> {
  let key = generate_master_key();
  create_new_stronghold(state, &key)?;
  let b64 = general_purpose::STANDARD.encode(key);
  app
    .keyring()
    .set_password(KR_SERVICE, KR_ACCOUNT, &b64)
    .map_err(|e| format!("failed to save key to keyring: {e}"))?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ai_api_key_name_is_stable() {
    // Stable contract: `stronghold_set` writes this name and the translation client reads it.
    assert_eq!(AI_API_KEY, "ai.apiKey");
    // The five auth keys must stay untouched (M1).
    assert_ne!(AI_API_KEY, "auth.bearer");
  }

  #[test]
  fn api_key_from_raw_trims_and_blank_is_missing() {
    assert!(api_key_from_raw(None).expect("none is valid").is_none());
    assert!(api_key_from_raw(Some(Vec::new()))
      .expect("empty is valid")
      .is_none());
    assert!(api_key_from_raw(Some(b"  \n\t ".to_vec()))
      .expect("blank is valid")
      .is_none());
  }

  #[test]
  fn api_key_from_raw_keeps_trimmed_secret() {
    let key = api_key_from_raw(Some(b"  sk-test-DO-NOT-LEAK\n".to_vec()))
      .expect("utf-8 is valid")
      .expect("non-blank is configured");
    assert_eq!(key.expose(), b"sk-test-DO-NOT-LEAK");
  }

  #[test]
  fn secrets_are_read_from_the_ovd_client_store() {
    let dir = std::env::temp_dir().join(format!("ovd-vault-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("vault.hold");

    let sh = Stronghold::new(&path, vec![7_u8; 32]).expect("create vault");
    sh.create_client(CLIENT).expect("create client");
    sh.write_client(CLIENT).expect("write client");
    sh.get_client(CLIENT)
      .expect("load client")
      .store()
      .insert(AI_API_KEY.as_bytes().to_vec(), b"sk-test".to_vec(), None)
      .expect("insert key");
    sh.get_client(CLIENT)
      .expect("load client")
      .store()
      .insert(b"auth.bearer".to_vec(), b"bearer-token".to_vec(), None)
      .expect("insert bearer");
    sh.save().expect("save vault");

    let state = StrongholdState::new(path);
    *state.inner.lock().unwrap() = Some(sh);

    let key = state
      .load_ai_api_key()
      .expect("read the key")
      .expect("configured");
    assert_eq!(key.expose(), b"sk-test");
    let secrets = state.load_auth_secrets().expect("read auth secrets");
    assert_eq!(secrets.bearer_token.as_deref(), Some("bearer-token"));

    let _ = std::fs::remove_dir_all(&dir);
  }
}
