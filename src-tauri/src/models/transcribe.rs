//! IPC payloads and terminal codes for the transcribe pipeline (design §7 IPC table).

use serde::{Deserialize, Serialize};

/// Stage names carried by `transcribe_stage`; the design's IPC table fixes these four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranscribeStage {
  DownloadingAudio,
  Transcribing,
  Translating,
  Writing,
}

impl TranscribeStage {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::DownloadingAudio => "downloadingAudio",
      Self::Transcribing => "transcribing",
      Self::Translating => "translating",
      Self::Writing => "writing",
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeStagePayload {
  pub id: String,
  pub group_id: String,
  pub stage: TranscribeStage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeProgressPayload {
  pub id: String,
  pub group_id: String,
  pub chunk_index: usize,
  pub chunk_total: usize,
  pub percent: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateProgressPayload {
  pub id: String,
  pub group_id: String,
  pub block_index: usize,
  pub block_total: usize,
  pub prompt_tokens: u64,
  pub completion_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
  En,
  Zh,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactWrittenPayload {
  pub id: String,
  pub group_id: String,
  pub kind: ArtifactKind,
  pub path: String,
}

/// Terminal status of one video in a batch (AC-19 / cut C7: skipped is a marker on `done`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoStatus {
  Done,
  Failed,
  Cancelled,
}

/// Stable error codes from design.md's error table (AC-24: every code has a log line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoErrorCode {
  #[serde(rename = "whisperMissing")]
  WhisperMissing,
  #[serde(rename = "whisperOutOfMemory")]
  WhisperOutOfMemory,
  #[serde(rename = "whisperFailed")]
  WhisperFailed,
  #[serde(rename = "ffmpegMissing")]
  FfmpegMissing,
  #[serde(rename = "ffmpegChunkFailed")]
  FfmpegChunkFailed,
  #[serde(rename = "durationUnknown")]
  DurationUnknown,
  #[serde(rename = "noSpeechDetected")]
  NoSpeechDetected,
  #[serde(rename = "deepseekAuthFailed")]
  DeepseekAuthFailed,
  #[serde(rename = "deepseekRateLimited")]
  DeepseekRateLimited,
  #[serde(rename = "deepseekServerError")]
  DeepseekServerError,
  #[serde(rename = "deepseekTimeout")]
  DeepseekTimeout,
  #[serde(rename = "translationContractViolation")]
  TranslationContractViolation,
  #[serde(rename = "outputWriteFailed")]
  OutputWriteFailed,
  #[serde(rename = "downloadFailed")]
  DownloadFailed,
  #[serde(rename = "fetchFailed")]
  FetchFailed,
}

impl VideoErrorCode {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::WhisperMissing => "whisperMissing",
      Self::WhisperOutOfMemory => "whisperOutOfMemory",
      Self::WhisperFailed => "whisperFailed",
      Self::FfmpegMissing => "ffmpegMissing",
      Self::FfmpegChunkFailed => "ffmpegChunkFailed",
      Self::DurationUnknown => "durationUnknown",
      Self::NoSpeechDetected => "noSpeechDetected",
      Self::DeepseekAuthFailed => "deepseekAuthFailed",
      Self::DeepseekRateLimited => "deepseekRateLimited",
      Self::DeepseekServerError => "deepseekServerError",
      Self::DeepseekTimeout => "deepseekTimeout",
      Self::TranslationContractViolation => "translationContractViolation",
      Self::OutputWriteFailed => "outputWriteFailed",
      Self::DownloadFailed => "downloadFailed",
      Self::FetchFailed => "fetchFailed",
    }
  }
}

impl std::fmt::Display for VideoErrorCode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

/// One row of `summary.md` and of the `batch_summary` event (AC-19).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummaryItem {
  pub group_id: String,
  pub url: String,
  pub status: VideoStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error_code: Option<VideoErrorCode>,
  /// Written artifacts (`transcript.en.txt` / `transcript.zh.txt`).
  pub outputs: Vec<String>,
  /// True when the two transcripts already existed and the video was skipped (cut C7).
  pub skipped: bool,
  pub prompt_tokens: u64,
  pub completion_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummaryPayload {
  pub path: String,
  pub items: Vec<BatchSummaryItem>,
}
