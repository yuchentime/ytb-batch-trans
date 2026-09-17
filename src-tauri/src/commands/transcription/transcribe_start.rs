//! `transcribe_start`: enqueues a batch of URLs into the transcribe pipeline (AC-01).

use crate::commands::transcription::transcription_probe::whisper_found;
use crate::logging::events;
use crate::scheduling::dispatcher::DispatchRequest;
use crate::scheduling::group_state::ensure_group_running;
use crate::scheduling::transcribe_pipeline::{
  TranscribeEntry, TranscribeRequest, TranscribeSender,
};
use crate::SharedConfig;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

/// Enqueues one transcribe group per URL and returns the group ids in order. The whisper
/// gate (AC-01) rejects the batch before any yt-dlp call when the environment is not ready.
#[tauri::command]
pub fn transcribe_start(
  urls: Vec<String>,
  app: AppHandle,
  pipeline: State<'_, TranscribeSender>,
) -> Result<Vec<String>, String> {
  let urls = urls
    .into_iter()
    .map(|url| url.trim().to_string())
    .filter(|url| !url.is_empty())
    .collect::<Vec<_>>();
  if urls.is_empty() {
    return Err("no urls provided".to_string());
  }
  if !whisper_found(&app) {
    return Err("whisperMissing".to_string());
  }

  let cfg = app.state::<SharedConfig>().load();
  let Some(root_dir) = cfg.output.root_dir.as_deref().map(PathBuf::from) else {
    return Err("output root directory is not configured".to_string());
  };

  let batch_id = Uuid::new_v4().to_string();
  let entries = urls
    .into_iter()
    .map(|url| {
      let group_id = Uuid::new_v4().to_string();
      ensure_group_running(&group_id);
      TranscribeEntry {
        batch_id: batch_id.clone(),
        group_id,
        id: Uuid::new_v4().to_string(),
        url,
      }
    })
    .collect::<Vec<_>>();
  let group_ids = entries
    .iter()
    .map(|entry| entry.group_id.clone())
    .collect::<Vec<_>>();

  pipeline
    .0
    .send(DispatchRequest::Pipeline(TranscribeRequest::Batch {
      batch_id: batch_id.clone(),
      root_dir,
      entries,
    }))
    .map_err(|error| error.to_string())?;

  tracing::info!(
    event = events::RUN_START,
    run = %batch_id,
    count = group_ids.len(),
  );
  Ok(group_ids)
}
