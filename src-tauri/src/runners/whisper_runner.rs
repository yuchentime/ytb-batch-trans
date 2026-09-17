//! whisper CLI runner for the transcribe pipeline (AC-03, AC-04).
//!
//! Builds the whisper argv, streams stdout segment lines as progress (the probe on
//! 2026-09-17 showed segment lines on stdout and tqdm on stderr), forwards stderr to the
//! caller for verbose logging, and maps exit codes / CUDA OOM to [`WhisperError`].
//! Spawning and cancellation reuse `ytdlp_process` (W001).

use crate::runners::ytdlp_process::{prepend_bin_dir_to_path, run_streaming, tail_excerpt};
use crate::state::config_models::{
  TranscriptionDevice, TranscriptionLanguage, TranscriptionModel, TranscriptionSettings,
};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::sync::watch;

const STDERR_EXCERPT_LINES: usize = 8;

#[derive(Debug)]
pub enum WhisperError {
  SpawnFailed(String),
  OutOfMemory { tail: String },
  Failed { exit: Option<i32>, tail: String },
  Cancelled,
}

impl fmt::Display for WhisperError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::SpawnFailed(error) => write!(f, "whisper failed to start: {error}"),
      Self::OutOfMemory { tail } => write!(f, "whisper ran out of GPU memory: {tail}"),
      Self::Failed { exit, tail } => write!(f, "whisper failed (exit {exit:?}): {tail}"),
      Self::Cancelled => write!(f, "cancelled"),
    }
  }
}

impl std::error::Error for WhisperError {}

/// One parsed `[start --> end]  text` line from whisper's stdout.
#[derive(Debug, Clone, PartialEq)]
pub struct WhisperSegment {
  pub start: f64,
  pub end: f64,
  pub text: String,
}

/// Builds the whisper argv for one chunk (design §3); `output_dir` receives `<chunk>.json`.
pub fn whisper_args(
  settings: &TranscriptionSettings,
  chunk: &Path,
  output_dir: &Path,
) -> Vec<String> {
  let fp16 = settings.fp16 && matches!(settings.device, TranscriptionDevice::Cuda);

  let mut args = vec![
    "--model".into(),
    model_arg(settings.model).into(),
    "--device".into(),
    device_arg(settings.device).into(),
    "--task".into(),
    "transcribe".into(),
    "--output_format".into(),
    "json".into(),
    "--verbose".into(),
    "True".into(),
    "--condition_on_previous_text".into(),
    bool_arg(settings.condition_on_previous_text).into(),
    "--fp16".into(),
    bool_arg(fp16).into(),
  ];

  if matches!(settings.language, TranscriptionLanguage::En) {
    args.push("--language".into());
    args.push("en".into());
  }

  args.push("--output_dir".into());
  args.push(output_dir.to_string_lossy().into_owned());
  args.push(chunk.to_string_lossy().into_owned());

  args
}

fn model_arg(model: TranscriptionModel) -> &'static str {
  match model {
    TranscriptionModel::Small => "small",
    TranscriptionModel::Medium => "medium",
    TranscriptionModel::LargeV3 => "large-v3",
  }
}

fn device_arg(device: TranscriptionDevice) -> &'static str {
  match device {
    TranscriptionDevice::Cuda => "cuda",
    TranscriptionDevice::Cpu => "cpu",
  }
}

fn bool_arg(value: bool) -> &'static str {
  if value {
    "True"
  } else {
    "False"
  }
}

/// Parses a whisper stdout segment line: `[00:00.000 --> 00:02.160]  text`.
/// Lines that do not match (tqdm, language detection, …) return `None`.
pub fn parse_segment_line(line: &str) -> Option<WhisperSegment> {
  let rest = line.trim_start().strip_prefix('[')?;
  let (timestamps, text) = rest.split_once(']')?;
  let (start, end) = timestamps.split_once("-->")?;
  let start = parse_timestamp(start.trim())?;
  let end = parse_timestamp(end.trim())?;
  if end < start {
    return None;
  }

  Some(WhisperSegment {
    start,
    end,
    text: text.trim().to_string(),
  })
}

fn parse_timestamp(value: &str) -> Option<f64> {
  let parts = value.split(':').collect::<Vec<_>>();
  match parts.len() {
    3 => {
      let hours = parse_clock_component(parts[0])?;
      let minutes = parse_clock_component(parts[1])?;
      let seconds = parse_clock_component(parts[2])?;
      Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }
    2 => {
      let minutes = parse_clock_component(parts[0])?;
      let seconds = parse_clock_component(parts[1])?;
      Some(minutes * 60.0 + seconds)
    }
    _ => None,
  }
}

fn parse_clock_component(value: &str) -> Option<f64> {
  let seconds = value.trim().parse::<f64>().ok()?;
  if seconds.is_finite() && seconds >= 0.0 {
    Some(seconds)
  } else {
    None
  }
}

pub fn is_out_of_memory(stderr: &str) -> bool {
  let lower = stderr.to_ascii_lowercase();
  lower.contains("cuda out of memory") || lower.contains("torch.cuda.outofmemoryerror")
}

/// Maps a non-zero whisper exit to the stable error codes (`whisperOutOfMemory` /
/// `whisperFailed`).
pub fn classify_whisper_failure(exit: Option<i32>, tail: String) -> WhisperError {
  if is_out_of_memory(&tail) {
    WhisperError::OutOfMemory { tail }
  } else {
    WhisperError::Failed { exit, tail }
  }
}

/// whisper writes `--output_format json` next to `--output_dir` as `<chunk basename>.json`.
pub fn chunk_json_path(chunk: &Path, output_dir: &Path) -> PathBuf {
  let stem = chunk
    .file_stem()
    .map(|stem| stem.to_os_string())
    .unwrap_or_default();
  // Append instead of `with_extension`: for `audio.000.m4a` that would treat `.000` as the
  // extension and produce `audio.json`.
  let mut file_name = stem;
  file_name.push(".json");
  output_dir.join(file_name)
}

/// Inputs for one `whisper` invocation. Grouped into a struct so the async entry point
/// stays under the argument-count lint while keeping the two stream callbacks explicit.
pub struct WhisperChunkRequest<'a> {
  /// whisper executable (or shim) to run.
  pub program: &'a Path,
  /// Toolchain directory prepended to `PATH` (whisper shells out to ffmpeg).
  pub bin_dir: &'a Path,
  pub settings: &'a TranscriptionSettings,
  /// Chunk to transcribe; the JSON output is named after its file stem.
  pub chunk: &'a Path,
  pub output_dir: &'a Path,
}

pub async fn transcribe_chunk(
  request: &WhisperChunkRequest<'_>,
  cancel: watch::Receiver<bool>,
  mut on_segment: impl FnMut(WhisperSegment),
  mut on_stderr_line: impl FnMut(&str),
) -> Result<PathBuf, WhisperError> {
  let json_path = chunk_json_path(request.chunk, request.output_dir);

  let mut command = Command::new(request.program);
  command.args(whisper_args(
    request.settings,
    request.chunk,
    request.output_dir,
  ));
  prepend_bin_dir_to_path(&mut command, request.bin_dir);

  let result = run_streaming(
    command,
    cancel,
    |line| {
      if let Some(segment) = parse_segment_line(line) {
        on_segment(segment);
      }
    },
    |line| on_stderr_line(line),
  )
  .await
  .map_err(WhisperError::SpawnFailed)?;

  if result.cancelled {
    return Err(WhisperError::Cancelled);
  }

  let tail = tail_excerpt(&result.stderr_tail, STDERR_EXCERPT_LINES);
  if result.code != Some(0) {
    return Err(classify_whisper_failure(result.code, tail));
  }
  if !json_path.exists() {
    return Err(WhisperError::Failed {
      exit: result.code,
      tail: format!("whisper produced no {} output", json_path.display()),
    });
  }

  Ok(json_path)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn settings() -> TranscriptionSettings {
    TranscriptionSettings::default()
  }

  #[test]
  fn default_args_match_the_design_contract() {
    let args = whisper_args(
      &settings(),
      Path::new("/out/.work/audio.000.m4a"),
      Path::new("/out/.work/chunks"),
    );

    assert_eq!(
      args,
      vec![
        "--model",
        "small",
        "--device",
        "cuda",
        "--task",
        "transcribe",
        "--output_format",
        "json",
        "--verbose",
        "True",
        "--condition_on_previous_text",
        "False",
        "--fp16",
        "True",
        "--language",
        "en",
        "--output_dir",
        "/out/.work/chunks",
        "/out/.work/audio.000.m4a",
      ]
    );
  }

  #[test]
  fn cpu_device_does_not_enable_fp16() {
    let settings = TranscriptionSettings {
      device: TranscriptionDevice::Cpu,
      ..settings()
    };
    let args = whisper_args(&settings, Path::new("chunk.m4a"), Path::new("out"));

    let fp16 = args.iter().position(|arg| arg == "--fp16").expect("--fp16");
    assert_eq!(args[fp16 + 1], "False");
  }

  #[test]
  fn language_auto_omits_the_language_flag() {
    let settings = TranscriptionSettings {
      language: TranscriptionLanguage::Auto,
      ..settings()
    };
    let args = whisper_args(&settings, Path::new("chunk.m4a"), Path::new("out"));

    assert!(!args.iter().any(|arg| arg == "--language"));
  }

  #[test]
  fn parses_whisper_stdout_segment_lines() {
    let segment =
      parse_segment_line("[00:01:02.500 --> 00:01:05.000]  Hello world").expect("segment line");

    assert_eq!(segment.start, 62.5);
    assert_eq!(segment.end, 65.0);
    assert_eq!(segment.text, "Hello world");
  }

  #[test]
  fn ignores_tqdm_and_non_segment_output() {
    assert!(parse_segment_line(" 50%|#####     | 3/10 [00:05<00:10, 1.2it/s]").is_none());
    assert!(parse_segment_line("Detected language: English").is_none());
    assert!(parse_segment_line("[not a timestamp] text").is_none());
    assert!(parse_segment_line("").is_none());
  }

  #[test]
  fn rejects_malformed_or_hostile_timestamps() {
    assert!(parse_segment_line("[inf:00 --> 00:01] x").is_none());
    assert!(parse_segment_line("[NaN:00 --> 00:01] x").is_none());
    assert!(parse_segment_line("[-1:00 --> 00:01] x").is_none());
    assert!(parse_segment_line("[00:01.000 --> 00:00.000] x").is_none());
  }

  #[test]
  fn detects_cuda_out_of_memory() {
    assert!(is_out_of_memory(
      "RuntimeError: CUDA out of memory. Tried to allocate 1.20 GiB"
    ));
    assert!(is_out_of_memory(
      "torch.cuda.OutOfMemoryError: CUDA out of memory."
    ));
    assert!(!is_out_of_memory("RuntimeError: unexpected EOF"));
  }

  #[test]
  fn classifies_oom_before_generic_failure() {
    assert!(matches!(
      classify_whisper_failure(Some(1), "CUDA out of memory".to_string()),
      WhisperError::OutOfMemory { .. }
    ));
    assert!(matches!(
      classify_whisper_failure(Some(1), "unexpected EOF".to_string()),
      WhisperError::Failed { exit: Some(1), .. }
    ));
  }

  #[test]
  fn chunk_json_path_uses_the_chunk_basename() {
    let json = chunk_json_path(
      Path::new("/out/.work/audio.000.m4a"),
      Path::new("/out/.work/chunks"),
    );

    assert_eq!(json, Path::new("/out/.work/chunks").join("audio.000.json"));
  }
}
