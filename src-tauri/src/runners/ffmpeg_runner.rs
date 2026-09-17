//! ffmpeg/ffprobe helpers for the transcribe pipeline.
//!
//! Duration probing (`ffprobe -show_entries format=duration`) and fixed-boundary chunk
//! cutting (`-ss` + `-t -c copy`). Spawning and cancellation reuse `ytdlp_process`
//! (hidden window, process group / Job Object, kill tree — W001); silence detection is
//! deliberately not performed (cut C1).

use crate::runners::ytdlp_process::{prepend_bin_dir_to_path, run_streaming, tail_excerpt};
use std::fmt;
use std::path::Path;
use std::process::Command;
use tokio::sync::watch;

const STDERR_EXCERPT_LINES: usize = 8;

#[derive(Debug)]
pub enum FfmpegError {
  SpawnFailed(String),
  ProbeFailed { exit: Option<i32>, tail: String },
  InvalidDuration,
  ChunkFailed { exit: Option<i32>, tail: String },
  Cancelled,
}

impl fmt::Display for FfmpegError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::SpawnFailed(error) => write!(f, "ffmpeg failed to start: {error}"),
      Self::ProbeFailed { exit, tail } => write!(f, "ffprobe failed (exit {exit:?}): {tail}"),
      Self::InvalidDuration => write!(f, "ffprobe returned no usable duration"),
      Self::ChunkFailed { exit, tail } => {
        write!(f, "ffmpeg chunk cut failed (exit {exit:?}): {tail}")
      }
      Self::Cancelled => write!(f, "cancelled"),
    }
  }
}

impl std::error::Error for FfmpegError {}

/// `ffprobe -v error -show_entries format=duration -of json <input>`
pub fn ffprobe_duration_args(input: &Path) -> Vec<String> {
  vec![
    "-v".into(),
    "error".into(),
    "-show_entries".into(),
    "format=duration".into(),
    "-of".into(),
    "json".into(),
    input.to_string_lossy().into_owned(),
  ]
}

/// Parses the `format.duration` value of ffprobe JSON output (ffprobe emits a string, a
/// number is accepted too). Zero/NaN/missing values are rejected.
pub fn parse_ffprobe_duration(stdout: &str) -> Result<f64, FfmpegError> {
  let value: serde_json::Value =
    serde_json::from_str(stdout.trim()).map_err(|_| FfmpegError::InvalidDuration)?;
  let duration = value
    .get("format")
    .and_then(|format| format.get("duration"))
    .and_then(parse_duration_value)
    .filter(|seconds| seconds.is_finite() && *seconds > 0.0);
  duration.ok_or(FfmpegError::InvalidDuration)
}

fn parse_duration_value(value: &serde_json::Value) -> Option<f64> {
  match value {
    serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
    serde_json::Value::Number(number) => number.as_f64(),
    _ => None,
  }
}

/// `ffmpeg -hide_banner -nostdin -y -ss <start> -i <input> -t <end - start> -c copy <output>`.
///
/// `-t` (duration) is used instead of the design's `-to`: `-to` combined with an input
/// `-ss` is interpreted against different timelines by different ffmpeg versions, which
/// could cut the wrong length. The `[start, end]` boundaries come from `plan_chunks`.
pub fn chunk_cut_args(start: f64, end: f64, input: &Path, output: &Path) -> Vec<String> {
  let duration = (end - start).max(0.0);
  vec![
    "-hide_banner".into(),
    "-nostdin".into(),
    "-y".into(),
    "-ss".into(),
    format_seconds(start),
    "-i".into(),
    input.to_string_lossy().into_owned(),
    "-t".into(),
    format_seconds(duration),
    "-c".into(),
    "copy".into(),
    output.to_string_lossy().into_owned(),
  ]
}

fn format_seconds(value: f64) -> String {
  format!("{value:.3}")
}

pub async fn probe_duration(
  program: &Path,
  bin_dir: &Path,
  input: &Path,
  cancel: watch::Receiver<bool>,
) -> Result<f64, FfmpegError> {
  let mut command = Command::new(program);
  command.args(ffprobe_duration_args(input));
  prepend_bin_dir_to_path(&mut command, bin_dir);

  let result = run_streaming(command, cancel, |_| {}, |_| {})
    .await
    .map_err(FfmpegError::SpawnFailed)?;
  if result.cancelled {
    return Err(FfmpegError::Cancelled);
  }
  if result.code != Some(0) {
    return Err(FfmpegError::ProbeFailed {
      exit: result.code,
      tail: tail_excerpt(&result.stderr_tail, STDERR_EXCERPT_LINES),
    });
  }

  parse_ffprobe_duration(&result.stdout)
}

pub async fn cut_chunk(
  program: &Path,
  bin_dir: &Path,
  start: f64,
  end: f64,
  input: &Path,
  output: &Path,
  cancel: watch::Receiver<bool>,
) -> Result<(), FfmpegError> {
  let mut command = Command::new(program);
  command.args(chunk_cut_args(start, end, input, output));
  prepend_bin_dir_to_path(&mut command, bin_dir);

  let result = run_streaming(command, cancel, |_| {}, |_| {})
    .await
    .map_err(FfmpegError::SpawnFailed)?;
  if result.cancelled {
    return Err(FfmpegError::Cancelled);
  }
  if result.code != Some(0) {
    return Err(FfmpegError::ChunkFailed {
      exit: result.code,
      tail: tail_excerpt(&result.stderr_tail, STDERR_EXCERPT_LINES),
    });
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ffprobe_args_target_format_duration_as_json() {
    let args = ffprobe_duration_args(Path::new("/tmp/audio.m4a"));

    assert_eq!(
      args,
      vec![
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "json",
        "/tmp/audio.m4a",
      ]
    );
  }

  #[test]
  fn parses_duration_from_string_and_number() {
    assert_eq!(
      parse_ffprobe_duration(r#"{"format":{"duration":"1200.500000"}}"#).expect("duration"),
      1200.5
    );
    assert_eq!(
      parse_ffprobe_duration(r#"{"format":{"duration":42}}"#).expect("duration"),
      42.0
    );
  }

  #[test]
  fn rejects_missing_or_invalid_duration() {
    assert!(parse_ffprobe_duration("").is_err());
    assert!(parse_ffprobe_duration("not json").is_err());
    assert!(parse_ffprobe_duration(r#"{"format":{"duration":"N/A"}}"#).is_err());
    assert!(parse_ffprobe_duration(r#"{"format":{"duration":"0"}}"#).is_err());
    assert!(parse_ffprobe_duration(r#"{"streams":[]}"#).is_err());
  }

  #[test]
  fn chunk_cut_uses_fixed_boundaries_and_stream_copy() {
    let args = chunk_cut_args(
      1200.0,
      2400.0,
      Path::new("/tmp/audio.m4a"),
      Path::new("/tmp/chunks/audio.001.m4a"),
    );

    assert_eq!(args[3], "-ss");
    assert_eq!(args[4], "1200.000");
    assert_eq!(args[5], "-i");
    assert_eq!(args[6], "/tmp/audio.m4a");
    assert_eq!(args[7], "-t");
    assert_eq!(args[8], "1200.000");
    assert_eq!(args[9], "-c");
    assert_eq!(args[10], "copy");
    assert_eq!(args[11], "/tmp/chunks/audio.001.m4a");
  }

  #[test]
  fn chunk_cut_never_scans_for_silence() {
    let args = chunk_cut_args(0.0, 60.0, Path::new("in.m4a"), Path::new("out.m4a"));

    for forbidden in ["silencedetect", "-af", "-filter_complex", "-vf"] {
      assert!(
        !args.iter().any(|arg| arg == forbidden),
        "unexpected {forbidden}"
      );
    }
  }
}
