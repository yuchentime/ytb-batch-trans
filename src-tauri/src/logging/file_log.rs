//! File logging for the transcribe pipeline (design §8).
//!
//! One global rotating log lives at `<app_dir>/logs/transcribe.log`. The layer is registered
//! next to the existing fmt/Sentry layers, so every `tracing` event is captured. The line
//! format is `<ISO8601 local> | <LEVEL> | <event> | run=… group=… stage=… <k=v …>`.
//!
//! Deviation from design §8 (recorded in `design.md` → Design Deviations): the writer is
//! synchronous instead of a `tracing-appender` non-blocking worker, because
//! `tracing-appender` only supports time-based rotation while AC-25 requires a 5 MB file cap.
//! Writes are one small `write(2)` per line (no fsync), so the pipeline is not meaningfully
//! blocked.

use crate::logging::events;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt::format::{FormatFields, Writer};
use tracing_subscriber::fmt::time::{FormatTime, LocalTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, MakeWriter};
use tracing_subscriber::registry::{LookupSpan, Registry};
use tracing_subscriber::Layer;

pub const LOG_DIR_NAME: &str = "logs";
pub const LOG_FILE_NAME: &str = "transcribe.log";
/// Per-file cap for the size-based rotation (design §8).
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Total number of files kept: the current one plus `.1` … `.4`.
pub const MAX_FILES: usize = 5;
/// Target used for the verbose raw output of yt-dlp/whisper/ffmpeg (never sent to Sentry).
pub const TOOL_OUTPUT_TARGET: &str = "ovd::tool_output";
pub const REDACTED: &str = "[redacted]";
pub const TITLE_MAX_CHARS: usize = 80;
pub const RESPONSE_MAX_CHARS: usize = 500;
pub const STDERR_MAX_CHARS: usize = 2048;
/// Hard safety cap for any other field; full URLs must survive (W008 decision A).
pub const FIELD_MAX_CHARS: usize = 8192;
#[allow(dead_code)] // Consumed by the runners (verbose raw output) in the next loops.
pub const TOOL_LINE_MAX_CHARS: usize = 2048;

const SENSITIVE_FIELD_NAMES: &[&str] = &[
  "apikey",
  "api_key",
  "authorization",
  "bearer",
  "bearertoken",
  "bearer_token",
  "cookie",
  "cookies",
  "headers",
  "password",
  "secret",
  "token",
  "videopassword",
  "video_password",
];

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn log_dir(app_dir: &Path) -> PathBuf {
  app_dir.join(LOG_DIR_NAME)
}

#[allow(dead_code)] // Returned by `transcription_probe` (AC-27) in the next loops.
pub fn log_file_path(app_dir: &Path) -> PathBuf {
  log_dir(app_dir).join(LOG_FILE_NAME)
}

#[allow(dead_code)] // Returned by `transcription_probe` (AC-27) in the next loops.
pub fn log_size_bytes(app_dir: &Path) -> u64 {
  match fs::metadata(log_file_path(app_dir)) {
    Ok(metadata) => metadata.len(),
    Err(_) => 0,
  }
}

/// Directory of the file log. Resolved lazily because `init_tracing` runs before
/// `PathsManager` exists; events emitted before [`configure`] are only dropped from the file.
#[derive(Default)]
pub struct LogDir(OnceLock<PathBuf>);

impl LogDir {
  fn get(&self) -> Option<&Path> {
    self.0.get().map(PathBuf::as_path)
  }

  fn set(&self, dir: PathBuf) {
    let _ = self.0.set(dir);
  }
}

static LOG_DIR: OnceLock<Arc<LogDir>> = OnceLock::new();

fn shared_log_dir() -> Arc<LogDir> {
  LOG_DIR.get_or_init(|| Arc::new(LogDir::default())).clone()
}

/// Points the file log at `<app_dir>/logs`; called once during setup.
pub fn configure(app_dir: &Path) {
  shared_log_dir().set(log_dir(app_dir));
}

// ---------------------------------------------------------------------------
// Sanitizing and redaction
// ---------------------------------------------------------------------------

/// Collapses line breaks/tabs to spaces and drops other control characters, so one event
/// always produces exactly one line.
pub fn sanitize_field(value: &str) -> String {
  let mut out = String::with_capacity(value.len());
  for ch in value.chars() {
    if ch == '\n' || ch == '\r' || ch == '\t' {
      out.push(' ');
    } else if !ch.is_control() {
      out.push(ch);
    }
  }
  out
}

/// Truncates to `max_chars` characters, appending `…` when cut (the marker is included in
/// the limit). Character-based, so multi-byte text is never split.
pub fn truncate_chars(value: &str, max_chars: usize) -> String {
  if value.chars().count() <= max_chars {
    return value.to_string();
  }
  let keep = max_chars.saturating_sub(1);
  let mut out: String = value.chars().take(keep).collect();
  out.push('…');
  out
}

#[allow(dead_code)] // Consumed by the pipeline when recording titles.
pub fn sanitize_title(value: &str) -> String {
  truncate_chars(&sanitize_field(value), TITLE_MAX_CHARS)
}

#[allow(dead_code)] // Consumed by the runners and the DeepSeek client for excerpts.
pub fn sanitize_excerpt(value: &str, max_chars: usize) -> String {
  truncate_chars(&sanitize_field(value), max_chars)
}

/// Field names whose values must never reach the file, even if a call site passes them.
pub fn is_sensitive_field(name: &str) -> bool {
  SENSITIVE_FIELD_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

fn field_limit(name: &str) -> usize {
  match name {
    "title" => TITLE_MAX_CHARS,
    "tail" => STDERR_MAX_CHARS,
    "reason" | "body" | "response" => RESPONSE_MAX_CHARS,
    _ => FIELD_MAX_CHARS,
  }
}

/// Emits one raw tool line; only written when `logging.verbose` is on.
#[allow(dead_code)] // Consumed by the runners in the next loops.
pub fn log_tool_output(verbose: bool, tool: &str, line: &str) {
  if !verbose {
    return;
  }
  let line = sanitize_excerpt(line, TOOL_LINE_MAX_CHARS);
  tracing::info!(
    target: TOOL_OUTPUT_TARGET,
    event = events::TOOL_OUTPUT,
    tool = %tool,
    line = %line
  );
}

// ---------------------------------------------------------------------------
// Line formatting
// ---------------------------------------------------------------------------

/// Formats one event per line: `<ISO8601 local> | <LEVEL> | <event> | run=… group=… stage=… k=v …`.
pub struct FileLogFormat {
  timer: LocalTime,
}

impl FileLogFormat {
  pub fn new() -> Self {
    Self {
      timer: LocalTime::rfc_3339(),
    }
  }
}

impl Default for FileLogFormat {
  fn default() -> Self {
    Self::new()
  }
}

impl<S, N> FormatEvent<S, N> for FileLogFormat
where
  S: Subscriber + for<'a> LookupSpan<'a>,
  N: for<'a> FormatFields<'a> + 'static,
{
  fn format_event(
    &self,
    _ctx: &FmtContext<'_, S, N>,
    mut writer: Writer<'_>,
    event: &Event<'_>,
  ) -> fmt::Result {
    let mut visitor = LogFieldVisitor::default();
    event.record(&mut visitor);

    self.timer.format_time(&mut writer)?;
    write!(writer, " | {} | ", event.metadata().level())?;

    let message = visitor.message.as_deref();
    match visitor.event.as_deref() {
      Some(code) => write!(writer, "{code}")?,
      None => {
        let fallback = message.unwrap_or_else(|| event.metadata().name());
        write!(writer, "{fallback}")?;
      }
    }

    write!(writer, " |")?;
    let ordered = [
      ("run", visitor.run.as_deref()),
      ("group", visitor.group.as_deref()),
      ("stage", visitor.stage.as_deref()),
    ];
    for (key, value) in ordered {
      if let Some(value) = value {
        write!(writer, " {key}={value}")?;
      }
    }
    for (key, value) in &visitor.fields {
      write!(writer, " {key}={value}")?;
    }
    if visitor.event.is_some() {
      if let Some(message) = message.filter(|value| !value.is_empty()) {
        write!(writer, " msg={message}")?;
      }
    }

    writeln!(writer)
  }
}

#[derive(Default)]
struct LogFieldVisitor {
  event: Option<String>,
  message: Option<String>,
  run: Option<String>,
  group: Option<String>,
  stage: Option<String>,
  fields: Vec<(String, String)>,
}

impl LogFieldVisitor {
  fn push(&mut self, name: &str, value: String) {
    let value = if is_sensitive_field(name) {
      REDACTED.to_string()
    } else {
      truncate_chars(&sanitize_field(&value), field_limit(name))
    };

    match name {
      "event" => self.event = Some(value),
      "message" => self.message = Some(value),
      "run" => self.run = Some(value),
      "group" => self.group = Some(value),
      "stage" => self.stage = Some(value),
      _ => self.fields.push((name.to_string(), value)),
    }
  }
}

impl Visit for LogFieldVisitor {
  fn record_str(&mut self, field: &Field, value: &str) {
    self.push(field.name(), value.to_string());
  }

  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    self.push(field.name(), format!("{value:?}"));
  }
}

// ---------------------------------------------------------------------------
// Size-rotating writer
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SizeRotatingFile {
  dir: Arc<LogDir>,
  state: Arc<Mutex<RotatingState>>,
  failure_reported: Arc<AtomicBool>,
  /// One formatted line is written as a single file write (flushed on newline/drop).
  buffer: Vec<u8>,
}

impl SizeRotatingFile {
  pub fn new(
    dir: Arc<LogDir>,
    file_name: impl Into<String>,
    max_bytes: u64,
    max_files: usize,
  ) -> Self {
    Self {
      dir,
      state: Arc::new(Mutex::new(RotatingState {
        file_name: file_name.into(),
        max_bytes,
        max_files: max_files.max(1),
        file: None,
        size: 0,
      })),
      failure_reported: Arc::new(AtomicBool::new(false)),
      buffer: Vec::new(),
    }
  }

  fn commit(&mut self) -> io::Result<()> {
    if self.buffer.is_empty() {
      return Ok(());
    }
    let result = self.write_buffered();
    self.buffer.clear();
    result
  }

  fn write_buffered(&self) -> io::Result<()> {
    let Some(dir) = self.dir.get() else {
      // Configured later during setup; drop silently instead of failing the pipeline.
      return Ok(());
    };
    let mut guard = lock_state(&self.state);
    let result = guard.write_line(dir, &self.buffer);
    drop(guard);
    if let Err(error) = &result {
      self.report_failure_once(error);
    }
    result
  }

  /// Degrades to a single warning per session; must never recurse (the failure flag is set
  /// before the warning is emitted, and the write path released the lock first).
  fn report_failure_once(&self, error: &io::Error) {
    if self.failure_reported.swap(true, Ordering::SeqCst) {
      return;
    }
    let dir = match self.dir.get() {
      Some(path) => path.to_string_lossy().into_owned(),
      None => String::new(),
    };
    tracing::warn!(
      event = events::LOG_WRITE_FAILED,
      dir = %dir,
      error = %error,
      "transcribe log write failed; file logging degraded"
    );
  }
}

struct RotatingState {
  file_name: String,
  max_bytes: u64,
  max_files: usize,
  file: Option<File>,
  size: u64,
}

fn lock_state(state: &Mutex<RotatingState>) -> MutexGuard<'_, RotatingState> {
  state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl RotatingState {
  fn write_line(&mut self, dir: &Path, bytes: &[u8]) -> io::Result<()> {
    if self.file.is_none() {
      self.open(dir)?;
    }
    let exceeds = self.size > 0 && self.size + bytes.len() as u64 > self.max_bytes;
    if exceeds {
      self.rotate(dir)?;
    }

    match self.file.as_mut() {
      Some(file) => {
        file.write_all(bytes)?;
        self.size += bytes.len() as u64;
        Ok(())
      }
      None => Err(io::Error::other("transcribe log file is not open")),
    }
  }

  fn open(&mut self, dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(&self.file_name);
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    self.size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    self.file = Some(file);
    Ok(())
  }

  fn rotate(&mut self, dir: &Path) -> io::Result<()> {
    self.file = None;

    if self.max_files <= 1 {
      let base = dir.join(&self.file_name);
      if base.exists() {
        fs::remove_file(&base)?;
      }
      self.size = 0;
      return self.open(dir);
    }

    for index in (1..self.max_files - 1).rev() {
      let from = rotated_path(dir, &self.file_name, index);
      let to = rotated_path(dir, &self.file_name, index + 1);
      move_file(&from, &to)?;
    }
    move_file(&dir.join(&self.file_name), &rotated_path(dir, &self.file_name, 1))?;
    self.size = 0;
    self.open(dir)
  }
}

fn rotated_path(dir: &Path, file_name: &str, index: usize) -> PathBuf {
  dir.join(format!("{file_name}.{index}"))
}

fn move_file(from: &Path, to: &Path) -> io::Result<()> {
  if !from.exists() {
    return Ok(());
  }
  if to.exists() {
    fs::remove_file(to)?;
  }
  fs::rename(from, to)
}

impl Write for SizeRotatingFile {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.buffer.extend_from_slice(buf);
    if self.buffer.contains(&b'\n') {
      self.commit()?;
    }
    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    // Every committed line is already written to the OS; no userspace buffer to flush.
    self.commit()
  }
}

impl Drop for SizeRotatingFile {
  fn drop(&mut self) {
    let _ = self.commit();
  }
}

impl<'a> MakeWriter<'a> for SizeRotatingFile {
  type Writer = SizeRotatingFile;

  fn make_writer(&'a self) -> Self::Writer {
    self.clone()
  }
}

// ---------------------------------------------------------------------------
// Layer wiring
// ---------------------------------------------------------------------------

/// File layer with the production writer (shared lazy dir, 5 MB × 5).
pub fn file_log_layer(default_level: LevelFilter) -> Box<dyn Layer<Registry> + Send + Sync> {
  let writer = SizeRotatingFile::new(shared_log_dir(), LOG_FILE_NAME, MAX_FILE_BYTES, MAX_FILES);
  file_log_layer_with(writer, default_level)
}

/// File layer with an injected writer (used by the L1 tests for rotation/threshold checks).
pub fn file_log_layer_with(
  writer: SizeRotatingFile,
  default_level: LevelFilter,
) -> Box<dyn Layer<Registry> + Send + Sync> {
  tracing_subscriber::fmt::layer()
    .event_format(FileLogFormat::new())
    .with_ansi(false)
    .with_writer(writer)
    .with_filter(file_layer_filter(default_level))
    .boxed()
}

fn file_layer_filter(default_level: LevelFilter) -> Targets {
  // Same noise exclusions as the fmt/Sentry layers.
  Targets::new()
    .with_target("tauri_plugin_updater", LevelFilter::OFF)
    .with_target("tao::platform_impl::platform::event_loop::runner", LevelFilter::OFF)
    .with_target("h2", LevelFilter::OFF)
    .with_target("hyper_util", LevelFilter::OFF)
    .with_default(default_level)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Mutex as StdMutex;
  use tracing_subscriber::prelude::*;

  fn temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ovd-{}-{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
  }

  fn format_events(dir: &Path, emit: impl FnOnce()) -> String {
    let log_dir = Arc::new(LogDir::default());
    log_dir.set(dir.to_path_buf());
    let writer = SizeRotatingFile::new(log_dir, LOG_FILE_NAME, MAX_FILE_BYTES, MAX_FILES);
    let layer = file_log_layer_with(writer, LevelFilter::DEBUG);
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, emit);
    fs::read_to_string(dir.join(LOG_FILE_NAME)).unwrap_or_default()
  }

  #[derive(Clone, Default)]
  struct BufferWriter(Arc<StdMutex<Vec<u8>>>);

  impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      let mut guard = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      guard.extend_from_slice(buf);
      Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  impl<'a> MakeWriter<'a> for BufferWriter {
    type Writer = BufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
      self.clone()
    }
  }

  fn capture_events(emit: impl FnOnce()) -> String {
    let writer = BufferWriter::default();
    let subscriber = tracing_subscriber::fmt()
      .with_writer(writer.clone())
      .with_ansi(false)
      .without_time()
      .finish();
    tracing::subscriber::with_default(subscriber, emit);
    let mutex = &writer.0;
    let guard = mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    String::from_utf8_lossy(guard.as_slice()).into_owned()
  }

  #[test]
  fn file_log_writes_one_line_with_stable_fields() {
    let dir = temp_dir("log-format");

    let content = format_events(&dir, || {
      tracing::info!(
        event = events::STAGE_CHANGE,
        run = "run-1",
        group = "group-1",
        stage = "transcribing",
        chunk = 3
      );
    });

    let lines = content.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains(" | INFO | stage.change |"));
    assert!(lines[0].contains("run=run-1 group=group-1 stage=transcribing chunk=3"));
  }

  #[test]
  fn file_log_appends_and_is_readable_without_shutdown() {
    let dir = temp_dir("log-append");

    format_events(&dir, || {
      tracing::info!(event = events::RUN_START, run = "run-1", count = 2);
    });
    let first = fs::read_to_string(dir.join(LOG_FILE_NAME)).expect("log file");
    assert!(first.contains("run.start"));

    let second = format_events(&dir, || {
      tracing::info!(event = events::RUN_END, run = "run-1");
    });
    assert!(second.contains("run.start"));
    assert!(second.contains("run.end"));
  }

  #[test]
  fn file_log_collapses_control_characters_and_truncates_titles() {
    let dir = temp_dir("log-truncate");
    let long_title = "T".repeat(200);

    let content = format_events(&dir, || {
      tracing::info!(
        event = events::TRANSCRIPT_EN_OK,
        group = "group-1",
        title = %format!("line one\nline two\t{long_title}")
      );
    });

    let line = content.lines().next().expect("one line");
    assert!(line.contains("title=line one line two T"));
    let value = line.split(" title=").nth(1).expect("title field");
    assert_eq!(value.chars().count(), TITLE_MAX_CHARS);
    assert!(value.ends_with('…'));
  }

  #[test]
  fn file_log_redacts_sensitive_fields_and_truncates_excerpts() {
    let dir = temp_dir("log-redact");
    let long_body = "B".repeat(900);

    let content = format_events(&dir, || {
      tracing::warn!(
        event = events::PROBE_MISSING,
        apiKey = "sk-test-DO-NOT-LEAK",
        password = "hunter2",
        whisper = "missing",
        reason = %long_body
      );
    });

    assert!(content.contains("apiKey=[redacted]"));
    assert!(content.contains("password=[redacted]"));
    assert!(!content.contains("sk-test-DO-NOT-LEAK"));
    assert!(!content.contains("hunter2"));
    assert!(content.contains("whisper=missing"));
    let reason = content.split(" reason=").nth(1).expect("reason field");
    assert_eq!(reason.trim_end().chars().count(), RESPONSE_MAX_CHARS);
    assert!(reason.trim_end().ends_with('…'));
  }

  #[test]
  fn rotation_keeps_the_newest_files_within_the_total_budget() {
    let dir = temp_dir("log-rotate");
    let log_dir = Arc::new(LogDir::default());
    log_dir.set(dir.clone());
    let mut writer = SizeRotatingFile::new(log_dir, LOG_FILE_NAME, 128, 3);

    for _ in 0..20 {
      writer.write_all(&[b'x'; 60]).expect("write payload");
      writer.write_all(b"\n").expect("write newline");
    }

    assert!(dir.join(LOG_FILE_NAME).exists());
    assert!(dir.join(format!("{LOG_FILE_NAME}.1")).exists());
    assert!(dir.join(format!("{LOG_FILE_NAME}.2")).exists());
    assert!(!dir.join(format!("{LOG_FILE_NAME}.3")).exists());

    let total = fs::read_dir(&dir)
      .expect("read dir")
      .filter_map(|entry| entry.ok())
      .filter_map(|entry| entry.metadata().ok())
      .map(|metadata| metadata.len())
      .sum::<u64>();
    assert!(total <= 3 * 128);
  }

  #[test]
  fn tool_output_is_only_logged_when_verbose() {
    let quiet_dir = temp_dir("log-quiet");
    let quiet = format_events(&quiet_dir, || {
      log_tool_output(false, "whisper", "raw line that must not appear");
    });
    assert!(!quiet.contains("raw line"));

    let verbose_dir = temp_dir("log-verbose");
    let verbose = format_events(&verbose_dir, || {
      log_tool_output(true, "whisper", "raw line that must appear");
    });
    assert!(verbose.contains("tool.output"));
    assert!(verbose.contains("tool=whisper"));
    assert!(verbose.contains("line=raw line that must appear"));
  }

  #[test]
  fn write_failure_warns_only_once_per_session() {
    let dir = temp_dir("log-fail");
    let file_as_parent = dir.join("not-a-directory");
    fs::write(&file_as_parent, b"x").expect("create blocking file");

    let log_dir = Arc::new(LogDir::default());
    log_dir.set(file_as_parent.join(LOG_DIR_NAME));
    let mut writer = SizeRotatingFile::new(log_dir, LOG_FILE_NAME, MAX_FILE_BYTES, MAX_FILES);

    let captured = capture_events(|| {
      assert!(writer.write_all(b"first line\n").is_err());
      assert!(writer.write_all(b"second line\n").is_err());
    });

    assert_eq!(captured.matches(events::LOG_WRITE_FAILED).count(), 1);
    assert!(captured.contains("dir="));
  }

  #[test]
  fn log_helpers_resolve_paths_below_app_dir() {
    let app_dir = temp_dir("log-paths");
    let file = log_file_path(&app_dir);
    assert_eq!(file, log_dir(&app_dir).join(LOG_FILE_NAME));
    assert_eq!(log_size_bytes(&app_dir), 0);

    fs::create_dir_all(log_dir(&app_dir)).expect("create log dir");
    fs::write(&file, b"12345").expect("write log file");
    assert_eq!(log_size_bytes(&app_dir), 5);
  }
}
