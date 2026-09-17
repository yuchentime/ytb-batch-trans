//! Video-level transcribe pipeline (design §1–§6): fetch metadata → download audio →
//! whisper chunks → merge → English transcript → DeepSeek translation → Chinese
//! transcript, with resume, cancellation, cleanup and the batch summary.
//!
//! Concurrency: the dispatcher's permit is the video slot (reusing `DownloadLimiter`);
//! inside a video, whisper is serialized by `TranscribeLimiter` (1) and each translation
//! block takes a `TranslateLimiter` permit.

use crate::logging::{events, file_log};
use crate::models::payloads::{MediaAddPayload, MediaFatalPayload};
use crate::models::progress::MediaProgressComplete;
use crate::models::transcribe::{
  ArtifactKind, ArtifactWrittenPayload, BatchSummaryItem, BatchSummaryPayload,
  TranscribeProgressPayload, TranscribeStage, TranscribeStagePayload, TranslateProgressPayload,
  VideoErrorCode, VideoStatus,
};
use crate::models::ParsedMedia;
use crate::paths::PathsManager;
use crate::runners::ffmpeg_runner::{cut_chunk, probe_duration, FfmpegError};
use crate::runners::whisper_runner::{transcribe_chunk, WhisperChunkRequest, WhisperError};
use crate::runners::ytdlp_args::build_audio_download_args;
use crate::runners::ytdlp_info::run_ytdlp_info_fetch;
use crate::runners::ytdlp_process::ProcessEvent;
use crate::runners::ytdlp_runner::YtdlpRunner;
use crate::scheduling::concurrency::DynamicSemaphore;
use crate::scheduling::dispatcher::{DispatchEntry, DispatchRequest, GenericDispatcher};
use crate::scheduling::group_state::subscribe_group;
use crate::state::config_models::Config;
use crate::stronghold::stronghold_state::StrongholdState;
use crate::transcribe::artifacts::{
  assemble_zh_transcript, find_audio_file, find_existing_output, load_segments, load_zh_blocks,
  store_segments, store_source_marker, store_zh_blocks, total_usage, write_atomic,
  BlockTranslation, OutputPaths, SourceMarker, SUMMARY_FILE_NAME,
};
use crate::transcribe::chunking::plan_chunks;
use crate::transcribe::merge::{merge_segments, ChunkSegments, MergedSegment, Segment};
use crate::transcribe::transcript::format_english_transcript;
use crate::translation::blocks::{plan_blocks, Block, BlockContext};
use crate::translation::deepseek_client::{DeepseekClient, DeepseekError, Usage};
use crate::translation::validate::check_numeric_fidelity;
use crate::{SharedConfig, TranscribeLimiter, TranslateLimiter};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;

#[derive(Clone)]
pub struct TranscribeSender(pub UnboundedSender<DispatchRequest<TranscribeRequest>>);

#[derive(Clone, Debug)]
pub enum TranscribeRequest {
  Batch {
    batch_id: String,
    root_dir: PathBuf,
    entries: Vec<TranscribeEntry>,
  },
}

/// One video in a transcribe batch. `id` is the media id used by the IPC payloads.
#[derive(Clone, Debug)]
pub struct TranscribeEntry {
  pub batch_id: String,
  pub group_id: String,
  pub id: String,
  pub url: String,
}

impl DispatchEntry for TranscribeEntry {
  fn group_id(&self) -> &String {
    &self.group_id
  }

  fn group_key(&self) -> Option<&String> {
    None
  }

  fn set_numbering(&mut self, _autonumber: u64, _group_autonumber: Option<u64>) {}
}

pub fn setup_transcribe_dispatcher(
  app: &AppHandle,
  sem: Arc<DynamicSemaphore>,
) -> GenericDispatcher<TranscribeRequest> {
  GenericDispatcher::start(
    app.clone(),
    sem,
    |req: TranscribeRequest| match req {
      TranscribeRequest::Batch {
        batch_id,
        root_dir,
        entries,
      } => {
        register_batch(&batch_id, &root_dir, entries.len());
        entries
          .into_iter()
          .map(|mut entry| {
            entry.batch_id = batch_id.clone();
            entry
          })
          .collect()
      }
    },
    |_tx, app: AppHandle, entry: TranscribeEntry| async move {
      let outcome = run_video(app.clone(), entry).await;
      record_outcome(&app, outcome);
    },
  )
}

struct BatchState {
  remaining: usize,
  root_dir: PathBuf,
  items: Vec<BatchSummaryItem>,
}

/// Batch bookkeeping; entries are removed when the last video of the batch reports.
static BATCHES: LazyLock<Mutex<HashMap<String, BatchState>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

fn register_batch(batch_id: &str, root_dir: &Path, total: usize) {
  BATCHES.lock().unwrap().insert(
    batch_id.to_string(),
    BatchState {
      remaining: total,
      root_dir: root_dir.to_path_buf(),
      items: Vec::new(),
    },
  );
}

fn record_outcome(app: &AppHandle, outcome: VideoOutcome) {
  let mut batches = BATCHES.lock().unwrap();
  let Some(state) = batches.get_mut(&outcome.entry.batch_id) else {
    return;
  };

  state.items.push(BatchSummaryItem {
    group_id: outcome.entry.group_id.clone(),
    url: outcome.entry.url.clone(),
    status: outcome.status,
    error_code: outcome.error_code,
    outputs: outcome.outputs.clone(),
    skipped: outcome.skipped,
    prompt_tokens: outcome.usage.prompt_tokens,
    completion_tokens: outcome.usage.completion_tokens,
  });
  state.remaining = state.remaining.saturating_sub(1);
  if state.remaining > 0 {
    return;
  }

  let Some(state) = batches.remove(&outcome.entry.batch_id) else {
    return;
  };
  drop(batches);

  let path = state.root_dir.join(SUMMARY_FILE_NAME);
  let markdown = render_summary(&state.items);
  if let Err(error) = write_atomic(&path, &markdown) {
    tracing::error!(run = %outcome.entry.batch_id, error = %error, "failed to write summary");
  }

  let done = state
    .items
    .iter()
    .filter(|item| item.status == VideoStatus::Done && !item.skipped)
    .count();
  let failed = state
    .items
    .iter()
    .filter(|item| item.status == VideoStatus::Failed)
    .count();
  let skipped = state.items.iter().filter(|item| item.skipped).count();
  tracing::info!(
    event = events::RUN_END,
    run = %outcome.entry.batch_id,
    count = state.items.len(),
    done = done,
    failed = failed,
    skipped = skipped,
  );

  let _ = app.emit(
    "batch_summary",
    BatchSummaryPayload {
      path: path.display().to_string(),
      items: state.items,
    },
  );
}

/// `summary.md` rows: status, outputs, failure code and token usage per video (AC-19).
pub fn render_summary(items: &[BatchSummaryItem]) -> String {
  let mut markdown = String::from(
    "# Transcribe summary\n\n\
     | # | URL | Status | Output | Error | Tokens prompt/completion |\n\
     | --- | --- | --- | --- | --- | --- |\n",
  );
  let (mut done, mut skipped, mut failed, mut cancelled) = (0_usize, 0, 0, 0);
  let (mut prompt, mut completion) = (0_u64, 0_u64);

  for (position, item) in items.iter().enumerate() {
    let status = if item.status == VideoStatus::Cancelled {
      cancelled += 1;
      "cancelled"
    } else if item.status == VideoStatus::Failed {
      failed += 1;
      "failed"
    } else if item.skipped {
      skipped += 1;
      "skipped"
    } else {
      done += 1;
      "done"
    };
    let outputs = if item.outputs.is_empty() {
      "-".to_string()
    } else {
      item.outputs.join("<br>")
    };
    let error = item.error_code.map(|code| code.as_str()).unwrap_or("-");
    markdown.push_str(&format!(
      "| {} | {} | {status} | {outputs} | {error} | {}/{} |\n",
      position + 1,
      item.url,
      item.prompt_tokens,
      item.completion_tokens,
    ));
    prompt += item.prompt_tokens;
    completion += item.completion_tokens;
  }

  markdown.push_str(&format!(
    "\nTotals: {} videos, {done} done, {skipped} skipped, {failed} failed, {cancelled} cancelled; \
     tokens {prompt}/{completion} (prompt/completion).\n",
    items.len()
  ));
  markdown
}

struct VideoOutcome {
  entry: TranscribeEntry,
  status: VideoStatus,
  error_code: Option<VideoErrorCode>,
  outputs: Vec<String>,
  skipped: bool,
  usage: Usage,
}

impl VideoOutcome {
  fn done(entry: TranscribeEntry, outputs: Vec<String>, skipped: bool, usage: Usage) -> Self {
    Self {
      entry,
      status: VideoStatus::Done,
      error_code: None,
      outputs,
      skipped,
      usage,
    }
  }

  fn failed(entry: TranscribeEntry, code: VideoErrorCode) -> Self {
    Self {
      entry,
      status: VideoStatus::Failed,
      error_code: Some(code),
      outputs: Vec::new(),
      skipped: false,
      usage: Usage::default(),
    }
  }

  fn cancelled(entry: TranscribeEntry) -> Self {
    Self {
      entry,
      status: VideoStatus::Cancelled,
      error_code: None,
      outputs: Vec::new(),
      skipped: false,
      usage: Usage::default(),
    }
  }
}

/// Failure inside one stage; `Cancelled` keeps the scene for a later resume.
enum JobError {
  Failed(VideoErrorCode, String),
  Cancelled,
}

impl JobError {
  fn failed(code: VideoErrorCode, message: impl Into<String>) -> Self {
    Self::Failed(code, message.into())
  }
}

async fn run_video(app: AppHandle, entry: TranscribeEntry) -> VideoOutcome {
  let cfg = app.state::<SharedConfig>().load();
  let Some(root_dir) = cfg.output.root_dir.as_deref().map(PathBuf::from) else {
    return fail_video(
      &app,
      &entry,
      "setup",
      VideoErrorCode::OutputWriteFailed,
      "output root directory is not configured",
    );
  };

  // AC-14: the already-exists check is local only — no yt-dlp, whisper or DeepSeek call.
  if !cfg.output.overwrite {
    if let Some(outputs) = find_existing_output(&root_dir, &entry.url) {
      let paths = vec![
        outputs.en.display().to_string(),
        outputs.zh.display().to_string(),
      ];
      tracing::info!(
        event = events::VIDEO_SKIP,
        run = %entry.batch_id,
        group = %entry.group_id,
        reason = "exists",
      );
      let _ = app.emit(
        "media_complete",
        MediaProgressComplete {
          id: entry.id.clone(),
          group_id: entry.group_id.clone(),
        },
      );
      return VideoOutcome::done(entry, paths, true, Usage::default());
    }
  }

  let mut cancel = subscribe_group(&entry.group_id);
  let single = match fetch_metadata(&app, &entry).await {
    Ok(single) => single,
    Err(error) => return finish_error(&app, &entry, "fetching", error),
  };
  let title = single.title.clone().unwrap_or_else(|| entry.id.clone());
  let outputs = OutputPaths::new(&root_dir, &title, cfg.output.restrict_filenames);

  if let Err(error) = store_source_marker(
    &outputs.work,
    &SourceMarker {
      url: entry.url.clone(),
      title: title.clone(),
    },
  ) {
    tracing::warn!(
      run = %entry.batch_id,
      group = %entry.group_id,
      error = %error,
      "failed to write the source marker"
    );
  }

  let result = run_stages(&app, &entry, &cfg, &outputs, single.duration, &mut cancel).await;
  match result {
    Ok((segments_count, skipped, usage)) => {
      tracing::info!(
        event = events::VIDEO_DONE,
        run = %entry.batch_id,
        group = %entry.group_id,
        segs = segments_count,
        outputs = %format!("{},{}", outputs.en.display(), outputs.zh.display()),
      );
      let _ = app.emit(
        "media_complete",
        MediaProgressComplete {
          id: entry.id.clone(),
          group_id: entry.group_id.clone(),
        },
      );
      VideoOutcome::done(
        entry,
        vec![
          outputs.en.display().to_string(),
          outputs.zh.display().to_string(),
        ],
        skipped,
        usage,
      )
    }
    Err(JobError::Cancelled) => {
      tracing::warn!(
        event = events::CANCEL_REQUESTED,
        run = %entry.batch_id,
        group = %entry.group_id,
        stage = "pipeline",
      );
      VideoOutcome::cancelled(entry)
    }
    Err(JobError::Failed(code, message)) => fail_video(&app, &entry, "pipeline", code, &message),
  }
}

fn finish_error(
  app: &AppHandle,
  entry: &TranscribeEntry,
  stage: &str,
  error: JobError,
) -> VideoOutcome {
  match error {
    JobError::Cancelled => VideoOutcome::cancelled(entry.clone()),
    JobError::Failed(code, message) => fail_video(app, entry, stage, code, &message),
  }
}

fn fail_video(
  app: &AppHandle,
  entry: &TranscribeEntry,
  stage: &str,
  code: VideoErrorCode,
  message: &str,
) -> VideoOutcome {
  tracing::error!(
    event = events::VIDEO_FAIL,
    run = %entry.batch_id,
    group = %entry.group_id,
    stage = stage,
    errCode = code.as_str(),
    message = %message,
  );
  let _ = app.emit(
    "media_fatal",
    MediaFatalPayload::business(
      entry.group_id.clone(),
      entry.id.clone(),
      format!("{}: {message}", code.as_str()),
      Some(code.as_str().to_string()),
    ),
  );
  VideoOutcome::failed(entry.clone(), code)
}

async fn fetch_metadata(
  app: &AppHandle,
  entry: &TranscribeEntry,
) -> Result<crate::models::ParsedSingleVideo, JobError> {
  let fetched = run_ytdlp_info_fetch(
    app,
    entry.id.clone(),
    entry.group_id.clone(),
    &entry.url,
    None,
    None,
  )
  .await;

  let single = match fetched {
    Ok(Some(ParsedMedia::Single(single))) => single,
    Ok(Some(_)) => {
      return Err(JobError::failed(
        VideoErrorCode::FetchFailed,
        "playlist and livestream links are expanded before the batch starts",
      ))
    }
    Ok(None) | Err(_) => {
      return Err(JobError::failed(
        VideoErrorCode::FetchFailed,
        "could not read video metadata",
      ))
    }
  };

  let _ = app.emit(
    "media_add",
    MediaAddPayload {
      group_id: entry.group_id.clone(),
      total: 1,
      item: single.clone(),
    },
  );
  Ok(single)
}

fn set_stage(app: &AppHandle, entry: &TranscribeEntry, stage: TranscribeStage) {
  tracing::info!(
    event = events::STAGE_CHANGE,
    run = %entry.batch_id,
    group = %entry.group_id,
    stage = stage.as_str(),
  );
  let _ = app.emit(
    "transcribe_stage",
    TranscribeStagePayload {
      id: entry.id.clone(),
      group_id: entry.group_id.clone(),
      stage,
    },
  );
}

fn is_cancelled(cancel: &watch::Receiver<bool>) -> bool {
  !*cancel.borrow()
}

type StageResult = Result<(usize, bool, Usage), JobError>;

/// Shared per-video inputs for the stage helpers (keeps argument counts small).
struct StageContext<'a> {
  app: &'a AppHandle,
  entry: &'a TranscribeEntry,
  cfg: &'a Arc<Config>,
  bin_dir: PathBuf,
}

/// Runs the four transcribe stages; `Err(Cancelled)` leaves the `.work` scene untouched.
async fn run_stages(
  app: &AppHandle,
  entry: &TranscribeEntry,
  cfg: &Arc<Config>,
  outputs: &OutputPaths,
  metadata_duration: Option<f64>,
  cancel: &mut watch::Receiver<bool>,
) -> StageResult {
  let ctx = StageContext {
    app,
    entry,
    cfg,
    bin_dir: app.state::<PathsManager>().bin_dir().clone(),
  };

  // --- downloadingAudio -------------------------------------------------------
  set_stage(app, entry, TranscribeStage::DownloadingAudio);
  let audio = download_audio(&ctx, outputs, cancel).await?;

  // --- transcribing -----------------------------------------------------------
  set_stage(app, entry, TranscribeStage::Transcribing);
  if is_cancelled(cancel) {
    return Err(JobError::Cancelled);
  }

  let segments_reused = !cfg.output.overwrite && load_segments(&outputs.segments).is_some();
  let segments = if segments_reused {
    load_segments(&outputs.segments).expect("checked above")
  } else {
    transcribe_segments(&ctx, &audio, metadata_duration, outputs, cancel).await?
  };

  if segments.is_empty() {
    tracing::info!(
      event = events::VIDEO_SKIP,
      run = %entry.batch_id,
      group = %entry.group_id,
      reason = "noSpeechDetected",
    );
    return Ok((0, true, Usage::default()));
  }

  let transcript = format_english_transcript(&segments);
  if let Err(error) = write_atomic(&outputs.en, &transcript.text) {
    return Err(JobError::failed(VideoErrorCode::OutputWriteFailed, error));
  }
  tracing::info!(
    event = events::TRANSCRIPT_EN_OK,
    run = %entry.batch_id,
    group = %entry.group_id,
    path = %outputs.en.display(),
    chars = transcript.char_count(),
    paragraphs = transcript.paragraph_count(),
  );
  emit_artifact(app, entry, ArtifactKind::En, &outputs.en);

  // --- translating ------------------------------------------------------------
  set_stage(app, entry, TranscribeStage::Translating);
  if is_cancelled(cancel) {
    return Err(JobError::Cancelled);
  }

  let plan = plan_blocks(&transcript.paragraphs, &cfg.translation);
  let mut blocks = load_zh_blocks(&outputs.zh_blocks).unwrap_or_default();
  let blocks_complete = crate::transcribe::artifacts::zh_blocks_complete(&blocks, &plan);
  let skip_translation = segments_reused && blocks_complete && !cfg.output.overwrite;

  if !skip_translation {
    translate_blocks(
      &ctx,
      outputs,
      &transcript.paragraphs,
      &plan,
      &mut blocks,
      cancel,
    )
    .await?;
  }

  // --- writing ----------------------------------------------------------------
  set_stage(app, entry, TranscribeStage::Writing);
  if is_cancelled(cancel) {
    return Err(JobError::Cancelled);
  }

  let Some(zh) = assemble_zh_transcript(&plan, &blocks) else {
    return Err(JobError::failed(
      VideoErrorCode::TranslationContractViolation,
      "stored block translations do not cover the planned paragraphs",
    ));
  };
  if let Err(error) = write_atomic(&outputs.zh, &zh) {
    return Err(JobError::failed(VideoErrorCode::OutputWriteFailed, error));
  }
  tracing::info!(
    event = events::TRANSCRIPT_ZH_OK,
    run = %entry.batch_id,
    group = %entry.group_id,
    path = %outputs.zh.display(),
    chars = zh.chars().count(),
  );
  emit_artifact(app, entry, ArtifactKind::Zh, &outputs.zh);

  cleanup_audio(app, entry, cfg, outputs, &audio);

  Ok((segments.len(), false, total_usage(&blocks)))
}

fn emit_artifact(app: &AppHandle, entry: &TranscribeEntry, kind: ArtifactKind, path: &Path) {
  let _ = app.emit(
    "artifact_written",
    ArtifactWrittenPayload {
      id: entry.id.clone(),
      group_id: entry.group_id.clone(),
      kind,
      path: path.display().to_string(),
    },
  );
}

async fn download_audio(
  ctx: &StageContext<'_>,
  outputs: &OutputPaths,
  cancel: &mut watch::Receiver<bool>,
) -> Result<PathBuf, JobError> {
  let StageContext {
    app, entry, cfg, ..
  } = ctx;
  let output_template = outputs.work.join("audio.%(ext)s").display().to_string();
  let secrets = app
    .state::<StrongholdState>()
    .load_auth_secrets()
    .unwrap_or_default();
  let args = build_audio_download_args(cfg, None, &output_template, &secrets);

  tracing::info!(
    event = events::AUDIO_DOWNLOAD_START,
    run = %entry.batch_id,
    group = %entry.group_id,
    url = %entry.url,
    path = %outputs.work.display(),
  );

  let runner = YtdlpRunner::new(app).with_args(args).with_url(&entry.url);
  let (mut rx, child) = runner.spawn().map_err(|error| {
    JobError::failed(
      VideoErrorCode::DownloadFailed,
      format!("spawn yt-dlp: {error}"),
    )
  })?;

  let verbose = cfg.logging.verbose;
  let exit_code;
  loop {
    if is_cancelled(cancel) {
      let _ = child.kill_tree();
      return Err(JobError::Cancelled);
    }

    tokio::select! {
      event = rx.recv() => {
        let Some(event) = event else {
          return Err(JobError::failed(VideoErrorCode::DownloadFailed, "yt-dlp event stream ended"));
        };
        match event {
          ProcessEvent::Stdout(line) => {
            let text = String::from_utf8_lossy(&line);
            file_log::log_tool_output(verbose, "yt-dlp", &text);
          }
          ProcessEvent::Stderr(line) => {
            let text = String::from_utf8_lossy(&line);
            file_log::log_tool_output(verbose, "yt-dlp", &text);
          }
          ProcessEvent::Terminated(payload) => {
            exit_code = payload.code;
            break;
          }
          ProcessEvent::Error(error) => {
            return Err(JobError::failed(VideoErrorCode::DownloadFailed, error));
          }
        }
      }
      _ = cancel.changed() => {
        if is_cancelled(cancel) {
          let _ = child.kill_tree();
          return Err(JobError::Cancelled);
        }
      }
    }
  }

  if exit_code != Some(0) {
    return Err(JobError::failed(
      VideoErrorCode::DownloadFailed,
      format!("yt-dlp exited with code {exit_code:?}"),
    ));
  }

  let audio = find_audio_file(&outputs.work).ok_or_else(|| {
    JobError::failed(
      VideoErrorCode::DownloadFailed,
      "yt-dlp produced no audio file",
    )
  })?;
  tracing::info!(
    event = events::AUDIO_DOWNLOAD_OK,
    run = %entry.batch_id,
    group = %entry.group_id,
    path = %audio.display(),
  );
  Ok(audio)
}

async fn transcribe_segments(
  ctx: &StageContext<'_>,
  audio: &Path,
  metadata_duration: Option<f64>,
  outputs: &OutputPaths,
  cancel: &mut watch::Receiver<bool>,
) -> Result<Vec<MergedSegment>, JobError> {
  let StageContext {
    app,
    entry,
    cfg,
    bin_dir,
  } = ctx;
  let duration = match metadata_duration.filter(|value| value.is_finite() && *value > 0.0) {
    Some(duration) => {
      tracing::info!(
        event = events::DURATION_RESOLVED,
        run = %entry.batch_id,
        group = %entry.group_id,
        duration = format!("{duration:.3}"),
      );
      duration
    }
    None => match probe_duration(Path::new("ffprobe"), bin_dir, audio, cancel.clone()).await {
      Ok(duration) => {
        tracing::info!(
          event = events::DURATION_RESOLVED,
          run = %entry.batch_id,
          group = %entry.group_id,
          duration = format!("{duration:.3}"),
        );
        duration
      }
      Err(FfmpegError::SpawnFailed(error)) => {
        tracing::error!(
          event = events::DURATION_UNKNOWN,
          run = %entry.batch_id,
          group = %entry.group_id,
          error = %error,
        );
        return Err(JobError::failed(VideoErrorCode::FfmpegMissing, error));
      }
      Err(error) => {
        tracing::error!(
          event = events::DURATION_UNKNOWN,
          run = %entry.batch_id,
          group = %entry.group_id,
          error = %error,
        );
        return Err(JobError::failed(
          VideoErrorCode::DurationUnknown,
          error.to_string(),
        ));
      }
    },
  };

  let transcription = cfg.transcription.clone();
  let chunks = plan_chunks(duration, transcription.chunk_minutes)
    .map_err(|error| JobError::failed(VideoErrorCode::DurationUnknown, error.to_string()))?;
  tracing::info!(
    event = events::CHUNK_PLAN,
    run = %entry.batch_id,
    group = %entry.group_id,
    chunks = chunks.len(),
    minutes = transcription.chunk_minutes,
  );

  fs::create_dir_all(&outputs.chunks).map_err(|error| {
    JobError::failed(
      VideoErrorCode::OutputWriteFailed,
      format!("create chunk dir: {error}"),
    )
  })?;

  let transcribe_slot = app.state::<TranscribeLimiter>().0.clone();
  let mut chunk_segments: Vec<ChunkSegments> = Vec::new();

  for chunk in &chunks {
    if is_cancelled(cancel) {
      return Err(JobError::Cancelled);
    }

    let _permit = transcribe_slot.acquire_owned().await;
    let chunk_path = outputs.chunks.join(format!("audio.{:03}.m4a", chunk.index));

    if let Err(error) = cut_chunk(
      Path::new("ffmpeg"),
      bin_dir,
      chunk.start,
      chunk.end,
      audio,
      &chunk_path,
      cancel.clone(),
    )
    .await
    {
      let (code, message) = ffmpeg_failure(&error);
      tracing::error!(
        event = events::CHUNK_CUT_FAIL,
        run = %entry.batch_id,
        group = %entry.group_id,
        idx = chunk.index,
        exit = format!("{:?}", exit_of_ffmpeg(&error)),
        error = %message,
      );
      return Err(JobError::failed(code, message));
    }
    tracing::info!(
      event = events::CHUNK_CUT_OK,
      run = %entry.batch_id,
      group = %entry.group_id,
      idx = chunk.index,
      start = format!("{:.3}", chunk.start),
      end = format!("{:.3}", chunk.end),
    );

    tracing::info!(
      event = events::WHISPER_START,
      run = %entry.batch_id,
      group = %entry.group_id,
      idx = chunk.index,
      total = chunks.len(),
      model = transcription_model(&transcription),
      device = transcription_device(&transcription),
    );
    let started = std::time::Instant::now();

    let program = transcription
      .whisper_path
      .as_deref()
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from("whisper"));
    let request = WhisperChunkRequest {
      program: &program,
      bin_dir,
      settings: &transcription,
      chunk: &chunk_path,
      output_dir: &outputs.chunks,
    };

    let span = (chunk.end - chunk.start).max(1.0);
    let verbose = cfg.logging.verbose;
    let progress_entry = (*entry).clone();
    let progress_app = (*app).clone();
    let chunk_index = chunk.index;
    let chunk_total = chunks.len();

    let json_path = match transcribe_chunk(
      &request,
      cancel.clone(),
      |segment| {
        let percent = ((segment.end / span) * 100.0).clamp(0.0, 100.0) as f32;
        let _ = progress_app.emit(
          "transcribe_progress",
          TranscribeProgressPayload {
            id: progress_entry.id.clone(),
            group_id: progress_entry.group_id.clone(),
            chunk_index,
            chunk_total,
            percent,
          },
        );
      },
      |line| file_log::log_tool_output(verbose, "whisper", line),
    )
    .await
    {
      Ok(json_path) => json_path,
      Err(WhisperError::Cancelled) => return Err(JobError::Cancelled),
      Err(error) => {
        let code = whisper_failure_code(&error);
        let message = error.to_string();
        let event = if code == VideoErrorCode::WhisperOutOfMemory {
          events::WHISPER_OOM
        } else {
          events::WHISPER_FAIL
        };
        tracing::error!(
          event = event,
          run = %entry.batch_id,
          group = %entry.group_id,
          idx = chunk_index,
          error = %message,
        );
        return Err(JobError::failed(code, message));
      }
    };

    let segments = match read_whisper_segments(&json_path) {
      Ok(segments) => segments,
      Err(error) => {
        tracing::error!(
          event = events::WHISPER_FAIL,
          run = %entry.batch_id,
          group = %entry.group_id,
          idx = chunk_index,
          error = %error,
        );
        return Err(JobError::failed(VideoErrorCode::WhisperFailed, error));
      }
    };
    tracing::info!(
      event = events::WHISPER_OK,
      run = %entry.batch_id,
      group = %entry.group_id,
      idx = chunk_index,
      segs = segments.len(),
      secs = format!("{:.1}", started.elapsed().as_secs_f64()),
    );

    chunk_segments.push(ChunkSegments {
      offset: chunk.start,
      planned_end: chunk.end,
      segments,
    });
  }

  let outcome = merge_segments(&chunk_segments)
    .map_err(|error| JobError::failed(VideoErrorCode::WhisperFailed, error.to_string()))?;
  tracing::info!(
    event = events::MERGE_OK,
    run = %entry.batch_id,
    group = %entry.group_id,
    segs = outcome.segments.len(),
    covered = format!("{:.3}", outcome.covered_seconds),
  );
  for gap in &outcome.coverage_gaps {
    tracing::warn!(
      event = events::MERGE_COVERAGE_GAP,
      run = %entry.batch_id,
      group = %entry.group_id,
      idx = gap.chunk_index,
      gap = format!("{:.3}", gap.gap_seconds),
    );
  }
  for risk in &outcome.boundary_risks {
    tracing::warn!(
      event = events::MERGE_BOUNDARY_RISK,
      run = %entry.batch_id,
      group = %entry.group_id,
      idx = risk.chunk_index,
      tail = risk.tail_too_close,
      head = risk.head_too_early,
    );
  }

  store_segments(&outputs.segments, &outcome.segments)
    .map_err(|error| JobError::failed(VideoErrorCode::OutputWriteFailed, error))?;

  Ok(outcome.segments)
}

async fn translate_blocks(
  ctx: &StageContext<'_>,
  outputs: &OutputPaths,
  paragraphs: &[String],
  plan: &[Block],
  blocks: &mut BTreeMap<usize, BlockTranslation>,
  cancel: &mut watch::Receiver<bool>,
) -> Result<(), JobError> {
  let StageContext {
    app, entry, cfg, ..
  } = ctx;
  let client = DeepseekClient::new(&cfg.translation)
    .map_err(|error| JobError::failed(deepseek_failure_code(&error), error.to_string()))?;
  let translate_slot = app.state::<TranslateLimiter>().0.clone();
  let glossary = crate::translation::blocks::parse_glossary(&cfg.translation.glossary);

  // Paragraph translations produced so far, used as context for the next block.
  let mut translations = vec![String::new(); paragraphs.len()];
  for block in plan {
    if let Some(stored) = blocks.get(&block.index) {
      if stored.ids == block.ids() && stored.zh.len() == stored.ids.len() {
        for (id, text) in stored.ids.iter().zip(stored.zh.iter()) {
          if *id < translations.len() {
            translations[*id] = text.clone();
          }
        }
      }
    }
  }

  for (position, block) in plan.iter().enumerate() {
    let complete = blocks
      .get(&block.index)
      .is_some_and(|stored| stored.ids == block.ids() && stored.zh.len() == stored.ids.len());
    if complete {
      continue;
    }

    let _permit = translate_slot.acquire_owned().await;
    if is_cancelled(cancel) {
      return Err(JobError::Cancelled);
    }

    let api_key = match app.state::<StrongholdState>().load_ai_api_key() {
      Ok(Some(key)) => key,
      Ok(None) => {
        return Err(JobError::failed(
          VideoErrorCode::DeepseekAuthFailed,
          "no DeepSeek API key is configured",
        ))
      }
      Err(error) => return Err(JobError::failed(VideoErrorCode::DeepseekAuthFailed, error)),
    };

    let context = BlockContext::before(block, paragraphs, &translations);
    tracing::info!(
      event = events::TRANSLATE_BLOCK_START,
      run = %entry.batch_id,
      group = %entry.group_id,
      block = block.index,
      blocks = plan.len(),
      chars = block.chars(),
    );

    let translated = match client.translate_block(block, &context, &api_key).await {
      Ok(translated) => translated,
      Err(error) => {
        let code = deepseek_failure_code(&error);
        let message = error.to_string();
        tracing::error!(
          event = events::TRANSLATE_BLOCK_FAIL,
          run = %entry.batch_id,
          group = %entry.group_id,
          block = block.index,
          status = format!("{code}"),
          reason = %message,
        );
        return Err(JobError::failed(code, message));
      }
    };

    let zh = translated
      .items
      .iter()
      .map(|item| item.zh.clone())
      .collect::<Vec<_>>();
    let ids = translated
      .items
      .iter()
      .map(|item| item.id)
      .collect::<Vec<_>>();
    for (id, text) in ids.iter().zip(zh.iter()) {
      if *id < translations.len() {
        translations[*id] = text.clone();
      }
    }

    let source = block
      .items
      .iter()
      .map(|item| item.text.as_str())
      .collect::<Vec<_>>()
      .join(" ");
    let missing = check_numeric_fidelity(&source, &zh.join(" "), &glossary);
    if !missing.is_empty() {
      tracing::warn!(
        event = events::TRANSLATE_FIDELITY_WARNING,
        run = %entry.batch_id,
        group = %entry.group_id,
        block = block.index,
        missing = %missing.join(","),
      );
    }

    blocks.insert(
      block.index,
      BlockTranslation {
        ids,
        zh,
        usage: translated.usage,
      },
    );
    store_zh_blocks(&outputs.zh_blocks, blocks)
      .map_err(|error| JobError::failed(VideoErrorCode::OutputWriteFailed, error))?;

    tracing::info!(
      event = events::TRANSLATE_BLOCK_OK,
      run = %entry.batch_id,
      group = %entry.group_id,
      block = block.index,
      prompt = translated.usage.prompt_tokens,
      completion = translated.usage.completion_tokens,
    );
    let _ = app.emit(
      "translate_progress",
      TranslateProgressPayload {
        id: entry.id.clone(),
        group_id: entry.group_id.clone(),
        block_index: position,
        block_total: plan.len(),
        prompt_tokens: translated.usage.prompt_tokens,
        completion_tokens: translated.usage.completion_tokens,
      },
    );
  }

  Ok(())
}

fn cleanup_audio(
  app: &AppHandle,
  entry: &TranscribeEntry,
  cfg: &Arc<Config>,
  outputs: &OutputPaths,
  audio: &Path,
) {
  if cfg.transcription.keep_audio {
    tracing::info!(
      event = events::CLEANUP_SKIP,
      run = %entry.batch_id,
      group = %entry.group_id,
      reason = "keepAudio",
    );
    return;
  }

  match fs::remove_file(audio) {
    Ok(()) => {
      tracing::info!(
        event = events::CLEANUP_OK,
        run = %entry.batch_id,
        group = %entry.group_id,
        removed = %audio.display(),
      );
      let _ = outputs;
    }
    Err(error) => {
      tracing::warn!(
        event = events::CLEANUP_SKIP,
        run = %entry.batch_id,
        group = %entry.group_id,
        reason = %error,
      );
      let _ = app;
    }
  }
  let _ = (app, outputs);
}

fn ffmpeg_failure(error: &FfmpegError) -> (VideoErrorCode, String) {
  match error {
    FfmpegError::SpawnFailed(message) => (VideoErrorCode::FfmpegMissing, message.clone()),
    FfmpegError::Cancelled => (VideoErrorCode::FfmpegChunkFailed, error.to_string()),
    _ => (VideoErrorCode::FfmpegChunkFailed, error.to_string()),
  }
}

fn exit_of_ffmpeg(error: &FfmpegError) -> Option<i32> {
  match error {
    FfmpegError::ProbeFailed { exit, .. } | FfmpegError::ChunkFailed { exit, .. } => *exit,
    _ => None,
  }
}

fn whisper_failure_code(error: &WhisperError) -> VideoErrorCode {
  match error {
    WhisperError::OutOfMemory { .. } => VideoErrorCode::WhisperOutOfMemory,
    WhisperError::SpawnFailed(_) => VideoErrorCode::WhisperMissing,
    _ => VideoErrorCode::WhisperFailed,
  }
}

fn deepseek_failure_code(error: &DeepseekError) -> VideoErrorCode {
  match error {
    DeepseekError::AuthFailed => VideoErrorCode::DeepseekAuthFailed,
    DeepseekError::RateLimited => VideoErrorCode::DeepseekRateLimited,
    DeepseekError::Timeout => VideoErrorCode::DeepseekTimeout,
    DeepseekError::InvalidResponse { .. } => VideoErrorCode::TranslationContractViolation,
    DeepseekError::ServerError { .. }
    | DeepseekError::RequestRejected { .. }
    | DeepseekError::Network(_) => VideoErrorCode::DeepseekServerError,
  }
}

fn transcription_model(
  settings: &crate::state::config_models::TranscriptionSettings,
) -> &'static str {
  use crate::state::config_models::TranscriptionModel;
  match settings.model {
    TranscriptionModel::Small => "small",
    TranscriptionModel::Medium => "medium",
    TranscriptionModel::LargeV3 => "large-v3",
  }
}

fn transcription_device(
  settings: &crate::state::config_models::TranscriptionSettings,
) -> &'static str {
  use crate::state::config_models::TranscriptionDevice;
  match settings.device {
    TranscriptionDevice::Cuda => "cuda",
    TranscriptionDevice::Cpu => "cpu",
  }
}

#[derive(Debug, Deserialize)]
struct WhisperJson {
  #[serde(default)]
  segments: Vec<WhisperJsonSegment>,
}

#[derive(Debug, Deserialize)]
struct WhisperJsonSegment {
  start: f64,
  end: f64,
  #[serde(default)]
  text: String,
}

fn read_whisper_segments(path: &Path) -> Result<Vec<Segment>, String> {
  let text =
    fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
  let parsed: WhisperJson =
    serde_json::from_str(&text).map_err(|error| format!("parse {}: {error}", path.display()))?;
  Ok(
    parsed
      .segments
      .into_iter()
      .map(|segment| Segment {
        start: segment.start,
        end: segment.end,
        text: segment.text.trim().to_string(),
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item(
    url: &str,
    status: VideoStatus,
    skipped: bool,
    code: Option<VideoErrorCode>,
    tokens: (u64, u64),
  ) -> BatchSummaryItem {
    BatchSummaryItem {
      group_id: format!("group-{url}"),
      url: url.to_string(),
      status,
      error_code: code,
      outputs: vec![format!("{url}/transcript.en.txt")],
      skipped,
      prompt_tokens: tokens.0,
      completion_tokens: tokens.1,
    }
  }

  #[test]
  fn summary_markdown_lists_statuses_codes_and_token_totals() {
    let items = vec![
      item("https://a", VideoStatus::Done, false, None, (100, 50)),
      item("https://b", VideoStatus::Done, true, None, (0, 0)),
      item(
        "https://c",
        VideoStatus::Failed,
        false,
        Some(VideoErrorCode::WhisperOutOfMemory),
        (10, 5),
      ),
      item("https://d", VideoStatus::Cancelled, false, None, (1, 1)),
    ];

    let markdown = render_summary(&items);

    assert!(markdown.contains("| 1 | https://a | done |"));
    assert!(markdown.contains("| 2 | https://b | skipped |"));
    assert!(markdown.contains("whisperOutOfMemory"));
    assert!(markdown.contains("| 4 | https://d | cancelled |"));
    assert!(markdown.contains("4 videos, 1 done, 1 skipped, 1 failed, 1 cancelled"));
    assert!(markdown.contains("tokens 111/56"));
  }

  #[test]
  fn error_codes_match_the_design_table() {
    assert_eq!(
      whisper_failure_code(&WhisperError::OutOfMemory {
        tail: String::new()
      }),
      VideoErrorCode::WhisperOutOfMemory
    );
    assert_eq!(
      whisper_failure_code(&WhisperError::Failed {
        exit: Some(1),
        tail: String::new()
      }),
      VideoErrorCode::WhisperFailed
    );
    assert_eq!(
      deepseek_failure_code(&DeepseekError::AuthFailed),
      VideoErrorCode::DeepseekAuthFailed
    );
    assert_eq!(
      deepseek_failure_code(&DeepseekError::RateLimited),
      VideoErrorCode::DeepseekRateLimited
    );
    assert_eq!(
      deepseek_failure_code(&DeepseekError::InvalidResponse {
        reason: String::new()
      }),
      VideoErrorCode::TranslationContractViolation
    );
    assert_eq!(
      deepseek_failure_code(&DeepseekError::Timeout),
      VideoErrorCode::DeepseekTimeout
    );
    assert_eq!(
      deepseek_failure_code(&DeepseekError::ServerError { status: 503 }),
      VideoErrorCode::DeepseekServerError
    );
    assert_eq!(
      ffmpeg_failure(&FfmpegError::SpawnFailed("missing".into())).0,
      VideoErrorCode::FfmpegMissing
    );
    assert_eq!(
      ffmpeg_failure(&FfmpegError::InvalidDuration).0,
      VideoErrorCode::FfmpegChunkFailed
    );
  }

  #[test]
  fn whisper_json_parsing_trims_text_and_rejects_junk() {
    let dir = std::env::temp_dir().join(format!("ovd-whisper-json-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("audio.000.json");

    fs::write(
      &path,
      r#"{"text":"x","language":"en","segments":[{"id":0,"start":0.0,"end":1.5,"text":"  Hello  "},{"id":1,"start":1.5,"end":2.0,"text":"World"}]}"#,
    )
    .expect("write");
    let segments = read_whisper_segments(&path).expect("parse");
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].text, "Hello");
    assert_eq!(segments[1].end, 2.0);

    fs::write(&path, "{not json").expect("write");
    assert!(read_whisper_segments(&path).is_err());
  }

  #[test]
  fn outcome_constructors_keep_the_contract() {
    let entry = TranscribeEntry {
      batch_id: "batch".into(),
      group_id: "group".into(),
      id: "id".into(),
      url: "https://a".into(),
    };

    let done = VideoOutcome::done(
      entry.clone(),
      vec!["en".into()],
      false,
      Usage {
        prompt_tokens: 1,
        completion_tokens: 2,
      },
    );
    assert_eq!(done.status, VideoStatus::Done);
    assert_eq!(done.usage.completion_tokens, 2);

    let failed = VideoOutcome::failed(entry.clone(), VideoErrorCode::WhisperMissing);
    assert_eq!(failed.status, VideoStatus::Failed);
    assert_eq!(failed.error_code, Some(VideoErrorCode::WhisperMissing));

    let cancelled = VideoOutcome::cancelled(entry);
    assert_eq!(cancelled.status, VideoStatus::Cancelled);
    assert!(cancelled.outputs.is_empty());
  }
}
