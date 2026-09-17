//! Notification kinds and OS delivery.
//!
//! Lives outside `commands` so backend pipelines (the transcribe batch) can notify
//! without depending on the command layer; `commands::notify` is a thin adapter.

use crate::i18n::I18nManager;
use crate::state::config_models::NotificationBehavior;
use crate::SharedConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
  QueueAdded,
  QueueDownloading,
  QueueFinished,
  VideoFinished,
  PlaylistFinished,
  VideoReady,
  PlaylistReady,
  DownloadFailed,
  BatchFinished,
}

impl NotificationKind {
  pub fn kind_str(&self) -> &'static str {
    match self {
      Self::QueueAdded => "queueAdded",
      Self::QueueDownloading => "queueDownloading",
      Self::QueueFinished => "queueFinished",
      Self::VideoFinished => "videoFinished",
      Self::PlaylistFinished => "playlistFinished",
      Self::VideoReady => "videoReady",
      Self::PlaylistReady => "playlistReady",
      Self::DownloadFailed => "downloadFailed",
      Self::BatchFinished => "batchFinished",
    }
  }

  #[inline]
  pub fn title_key(&self) -> String {
    format!("notifications.{}.title", self.kind_str())
  }

  #[inline]
  pub fn body_key(&self) -> String {
    format!("notifications.{}.body", self.kind_str())
  }
}

/// Sends one notification, honouring `notifications.notificationBehavior` and
/// `notifications.disabledNotifications`.
///
/// Fails only when the OS notification itself cannot be shown; a missing app state
/// (unit tests) is a no-op instead of a panic.
pub fn notify<R: Runtime>(
  app: &AppHandle<R>,
  kind: NotificationKind,
  params: Option<HashMap<String, String>>,
  force: bool,
) -> Result<(), String> {
  let (Some(cfg_handle), Some(i18n)) = (
    app.try_state::<SharedConfig>(),
    app.try_state::<I18nManager>(),
  ) else {
    tracing::warn!("notify called before the app state was managed");
    return Ok(());
  };
  let cfg = cfg_handle.load();
  if cfg.notifications.disabled_notifications.contains(&kind) {
    return Ok(());
  }
  match cfg.notifications.notification_behavior {
    NotificationBehavior::Always => {}
    NotificationBehavior::Never => return Ok(()),
    NotificationBehavior::OnBackground => {
      if let Some(window) = app.get_webview_window("main") {
        if (window.is_visible().unwrap_or(false) || window.is_focused().unwrap_or(false)) && !force
        {
          return Ok(());
        }
      }
    }
  }

  let title = i18n.t_with(&kind.title_key(), params.as_ref());
  let body = i18n.t_with(&kind.body_key(), params.as_ref());

  #[cfg(target_os = "linux")]
  {
    use notify_rust::Notification;

    let handle = Notification::new()
      .summary(title.as_str())
      .body(body.as_str())
      .icon("open-video-downloader")
      .show()
      .map_err(|e| e.to_string())?;

    std::thread::spawn(move || {
      handle.on_close(|_| {});
    });
  }

  #[cfg(not(target_os = "linux"))]
  {
    use tauri_plugin_notification::NotificationExt;

    app
      .notification()
      .builder()
      .title(title)
      .body(body)
      .show()
      .map_err(|e| e.to_string())?;
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  const ALL_KINDS: [NotificationKind; 9] = [
    NotificationKind::QueueAdded,
    NotificationKind::QueueDownloading,
    NotificationKind::QueueFinished,
    NotificationKind::VideoFinished,
    NotificationKind::PlaylistFinished,
    NotificationKind::VideoReady,
    NotificationKind::PlaylistReady,
    NotificationKind::DownloadFailed,
    NotificationKind::BatchFinished,
  ];

  #[test]
  fn every_kind_has_a_backend_and_frontend_locale_entry() {
    let backend: serde_json::Value =
      serde_json::from_str(include_str!("../locales/en.json")).expect("backend locale");
    let frontend: serde_json::Value =
      serde_json::from_str(include_str!("../../src/locales/en.json")).expect("frontend locale");

    for kind in ALL_KINDS {
      let key = kind.kind_str();
      assert!(
        backend["notifications"][key]["title"].is_string(),
        "missing notifications.{key}.title in src-tauri/locales/en.json"
      );
      assert!(
        backend["notifications"][key]["body"].is_string(),
        "missing notifications.{key}.body in src-tauri/locales/en.json"
      );
      assert!(
        frontend["settings"]["notifications"]["disabled"]["kinds"][key].is_string(),
        "missing settings.notifications.disabled.kinds.{key} in src/locales/en.json"
      );
    }
  }

  #[test]
  fn batch_finished_keys_follow_the_notification_contract() {
    let kind = NotificationKind::BatchFinished;
    assert_eq!(kind.kind_str(), "batchFinished");
    assert_eq!(kind.title_key(), "notifications.batchFinished.title");
    assert_eq!(kind.body_key(), "notifications.batchFinished.body");
  }
}
