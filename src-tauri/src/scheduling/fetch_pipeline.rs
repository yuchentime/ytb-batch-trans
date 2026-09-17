use crate::models::download::DownloadOverrides;
use crate::models::{MediaAddPayload, MediaFatalPayload, PlaylistEntry};
use crate::runners::ytdlp_info::{run_ytdlp_info_fetch, YtdlpInfoFetchError};
use crate::{
  models::ParsedMedia,
  scheduling::concurrency::DynamicSemaphore,
  scheduling::dispatcher::{DispatchEntry, DispatchRequest, GenericDispatcher},
};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Clone)]
pub struct FetchSender(pub UnboundedSender<DispatchRequest<FetchRequest>>);

#[derive(Clone)]
pub enum FetchRequest {
  Initial {
    group_id: String,
    id: String,
    url: String,
    overrides: Box<Option<DownloadOverrides>>,
  },
  Playlist {
    group_id: String,
    entries: Vec<PlaylistEntry>,
    overrides: Box<Option<DownloadOverrides>>,
  },
}

#[derive(Clone)]
pub struct FetchEntry {
  pub group_id: String,
  pub id: String,
  pub url: String,
  pub total: usize,
  pub overrides: Option<DownloadOverrides>,
}

impl DispatchEntry for FetchEntry {
  fn group_id(&self) -> &String {
    &self.group_id
  }
  fn group_key(&self) -> Option<&String> {
    None
  }
  fn set_numbering(&mut self, _autonumber: u64, _group_autonumber: Option<u64>) {}
}

static GROUP_COUNTERS: LazyLock<Mutex<HashMap<String, usize>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn setup_fetch_dispatcher(
  app: &AppHandle,
  sem: Arc<DynamicSemaphore>,
) -> GenericDispatcher<FetchRequest> {
  GenericDispatcher::start(
    app.clone(),
    sem,
    expand_fetch_request,
    |tx: UnboundedSender<DispatchRequest<FetchRequest>>, app: AppHandle, entry: FetchEntry| async move {
      handle_fetch_entry(tx, app, entry).await;
    },
  )
}

fn expand_fetch_request(req: FetchRequest) -> Vec<FetchEntry> {
  match req {
    FetchRequest::Initial {
      group_id,
      id,
      url,
      overrides,
    } => {
      vec![FetchEntry {
        group_id,
        id,
        url,
        total: 1,
        overrides: *overrides,
      }]
    }
    FetchRequest::Playlist {
      group_id,
      entries,
      overrides,
    } => {
      let total = entries.len();
      GROUP_COUNTERS
        .lock()
        .unwrap()
        .insert(group_id.clone(), total);
      entries
        .into_iter()
        .map(|e| FetchEntry {
          group_id: group_id.clone(),
          id: Uuid::new_v4().to_string(),
          url: e.video_url,
          total,
          overrides: *overrides.clone(),
        })
        .collect()
    }
  }
}

async fn handle_fetch_entry(
  tx: UnboundedSender<DispatchRequest<FetchRequest>>,
  app: AppHandle,
  entry: FetchEntry,
) {
  let FetchEntry {
    group_id,
    id,
    url,
    total,
    overrides,
  } = entry.clone();

  let result = run_ytdlp_info_fetch(
    &app,
    id.clone(),
    group_id.clone(),
    &url,
    None,
    overrides.clone(),
  )
  .await;

  let result = match result {
    Ok(v) => v,
    Err(e) => {
      tracing::warn!(
        fetch_id = %id,
        group_id = %group_id,
        url = %url,
        error = %e,
        "run_ytdlp_info_fetch failed"
      );
      if should_report_to_sentry(&e) {
        sentry::capture_error(&e);
      }

      None
    }
  };

  match result {
    Some(ParsedMedia::Single(single)) => {
      let payload = MediaAddPayload {
        group_id: group_id.clone(),
        total,
        item: single,
      };
      let _ = app.emit("media_add", payload);

      let mut counters = GROUP_COUNTERS.lock().unwrap();
      if let Some(cnt) = counters.get_mut(&group_id) {
        *cnt -= 1;
        if *cnt == 0 {
          counters.remove(&group_id);
          let _ = tx.send(DispatchRequest::Cleanup {
            group_id: group_id.clone(),
          });
        }
      }
    }
    Some(ParsedMedia::Playlist(pl)) => {
      let payload = MediaAddPayload {
        group_id,
        total: pl.entries.len(),
        item: pl,
      };
      let _ = app.emit("media_add", payload);
    }
    Some(ParsedMedia::Livestream(_)) => {
      let payload =
        MediaFatalPayload::internal(group_id.clone(), id, "Livestreams unsupported".into(), None);
      let _ = app.emit("media_fatal", payload);
      let mut counters = GROUP_COUNTERS.lock().unwrap();
      if let Some(cnt) = counters.get_mut(&group_id) {
        *cnt -= 1;
        if *cnt == 0 {
          counters.remove(&group_id);
          let _ = tx.send(DispatchRequest::Cleanup {
            group_id: group_id.clone(),
          });
        }
      }
    }
    None => {
      // Do nothing if no parsed result is returned. The events have already been sent.
    }
  }
}

fn should_report_to_sentry(err: &YtdlpInfoFetchError) -> bool {
  matches!(
    err,
    YtdlpInfoFetchError::InvalidDiagnosticRules(_)
      | YtdlpInfoFetchError::RunnerFailed(_)
      | YtdlpInfoFetchError::ParseFailed(_)
  )
}
