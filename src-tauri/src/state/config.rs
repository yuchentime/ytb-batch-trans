use crate::commands::{register_shortcuts, unregister_shortcuts};
use crate::i18n::I18nManager;
use crate::state::config_models::Config;
use crate::state::json_handle::JsonStoreHandle;
use crate::state::json_state::JsonBackedState;
use crate::tray::{create_tray, destroy_tray};
use crate::{DownloadLimiter, FetchLimiter};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

/// Directory (inside the host download dir) that holds transcripts and their `.work` artifacts.
const TRANSCRIPT_DIR_NAME: &str = "ovd-transcripts";

/// Fills path defaults that depend on the host environment. Kept separate from `Default` so the
/// derive stays environment independent, and so old config files keep loading unchanged (AC-18).
fn apply_path_defaults(value: &mut Config, base_dir: PathBuf) {
  if value.output.download_dir.is_none() {
    value.output.download_dir = Some(base_dir.to_string_lossy().into_owned());
  }
  if value.output.root_dir.is_none() {
    value.output.root_dir = Some(
      base_dir
        .join(TRANSCRIPT_DIR_NAME)
        .to_string_lossy()
        .into_owned(),
    );
  }
}

impl JsonBackedState for Config {
  const STORE_FILE: &'static str = "config.store.json";
  const ROOT_KEY: &'static str = "config";

  fn default_value() -> Self {
    Config::default()
  }

  fn before_initialized(app: &AppHandle<Wry>, value: &mut Self) {
    if value.network.enable_proxy.is_none() {
      value.network.enable_proxy =
        Some(value.network.proxy.as_ref().is_some_and(|v| !v.is_empty()));
    }
    if value.output.download_dir.is_none() || value.output.root_dir.is_none() {
      let base_dir = app
        .path()
        .download_dir()
        .ok()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
      apply_path_defaults(value, base_dir);
    }
  }

  fn on_updated(app: &AppHandle<Wry>, new_value: &Self) {
    if let Some(limiter) = app.try_state::<DownloadLimiter>() {
      let limiter = limiter.0.clone();
      let max = new_value.performance.max_concurrency;
      tauri::async_runtime::spawn(async move {
        limiter.resize(max).await;
      });
    }
    if let Some(limiter) = app.try_state::<FetchLimiter>() {
      let limiter = limiter.0.clone();
      let max = new_value.performance.max_concurrency;
      tauri::async_runtime::spawn(async move {
        limiter.resize(max).await;
      });
    }

    if new_value.input.global_shortcuts {
      register_shortcuts(app);
    } else {
      unregister_shortcuts(app);
    }

    if new_value.system.tray_enabled {
      create_tray(app);
    } else {
      destroy_tray(app);
    }

    if new_value.appearance.language == "system" {
      let i18n_handle = app.state::<I18nManager>();
      i18n_handle.unset_locale();
      if new_value.system.tray_enabled {
        destroy_tray(app);
        create_tray(app);
      }
    } else {
      let i18n_handle = app.state::<I18nManager>();
      i18n_handle.set_locale(&new_value.appearance.language);
      if new_value.system.tray_enabled {
        destroy_tray(app);
        create_tray(app);
      }
    }

    if new_value.system.auto_start_enabled {
      let _ = app.autolaunch().enable();
    } else {
      let _ = app.autolaunch().disable();
    }
  }
}

pub type ConfigHandle = JsonStoreHandle<Config>;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::config_models::{LoggingSettings, TranscriptionSettings, TranslationSettings};
  use std::path::Path;

  /// Snapshot of a pre-transcription `config.store.json`: download-era fields plus keys
  /// that later phases remove. Loading it must never fail (AC-18).
  const LEGACY_CONFIG: &str = r#"{
    "appearance": { "theme": "dark" },
    "input": { "preferVideoInMixedLinks": true },
    "inputFilters": { "minSize": { "value": 10, "unit": "MB" } },
    "output": {
      "downloadDir": "D:/legacy-downloads",
      "fileNameTemplate": "legacy-%(title)s.%(ext)s",
      "video": { "container": "mkv" }
    },
    "performance": { "autoLoadSize": false },
    "sponsorBlock": { "removeParts": ["sponsor"] },
    "subtitles": { "enabled": true },
    "keyRemovedInAFuturePhase": { "nested": true }
  }"#;

  #[test]
  fn materialize_ignores_unknown_keys_and_applies_new_defaults() {
    let raw: serde_json::Value = serde_json::from_str(LEGACY_CONFIG).expect("legacy json parses");
    let config = Config::materialize(&raw).expect("legacy config must load");

    // Known legacy values survive the deep merge.
    assert_eq!(config.appearance.theme, "dark");
    assert!(config.input.prefer_video_in_mixed_links);
    assert_eq!(
      config.output.download_dir.as_deref(),
      Some("D:/legacy-downloads")
    );
    assert_eq!(config.output.file_name_template, "legacy-%(title)s.%(ext)s");

    // Keys that no longer exist (or never existed) are ignored, not fatal.
    // New sections fall back to their defaults.
    assert_eq!(config.transcription, TranscriptionSettings::default());
    assert_eq!(config.translation, TranslationSettings::default());
    assert_eq!(config.logging, LoggingSettings::default());
    assert!(!config.output.overwrite);
    assert!(!config.output.restrict_filenames);
    // The environment-dependent path default is filled by `before_initialized`, not `Default`.
    assert_eq!(config.output.root_dir, None);
  }

  #[test]
  fn apply_path_defaults_fills_only_missing_paths() {
    let mut config = Config::default();
    apply_path_defaults(&mut config, PathBuf::from("D:/Downloads"));

    assert_eq!(config.output.download_dir.as_deref(), Some("D:/Downloads"));
    assert_eq!(
      config.output.root_dir.as_deref(),
      Path::new("D:/Downloads").join(TRANSCRIPT_DIR_NAME).to_str()
    );

    let mut configured = Config::default();
    configured.output.download_dir = Some("E:/keep".into());
    configured.output.root_dir = Some("E:/keep/transcripts".into());
    apply_path_defaults(&mut configured, PathBuf::from("D:/Downloads"));

    assert_eq!(configured.output.download_dir.as_deref(), Some("E:/keep"));
    assert_eq!(
      configured.output.root_dir.as_deref(),
      Some("E:/keep/transcripts")
    );
  }
}
