//! `notify` command: thin adapter over `crate::notifications::notify`.

pub use crate::notifications::NotificationKind;
use std::collections::HashMap;
use tauri::AppHandle;

#[tauri::command]
pub fn notify(
  app: AppHandle,
  kind: NotificationKind,
  params: Option<HashMap<String, String>>,
  force: bool,
) -> Result<(), String> {
  crate::notifications::notify(&app, kind, params, force)
}
