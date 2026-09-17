//! Stable event codes for the file log (design §8).
//!
//! These strings are a contract: they are grep targets for troubleshooting and tests
//! assert them verbatim. Add a code to the design table first, then here.

pub const RUN_START: &str = "run.start";
pub const RUN_END: &str = "run.end";
pub const PROBE_OK: &str = "probe.ok";
pub const PROBE_MISSING: &str = "probe.missing";
pub const STAGE_CHANGE: &str = "stage.change";
pub const PLAYLIST_EXPAND: &str = "playlist.expand";
pub const AUDIO_DOWNLOAD_START: &str = "audio.download.start";
pub const AUDIO_DOWNLOAD_OK: &str = "audio.download.ok";
pub const AUDIO_DOWNLOAD_FAIL: &str = "audio.download.fail";
pub const DURATION_RESOLVED: &str = "duration.resolved";
pub const DURATION_UNKNOWN: &str = "duration.unknown";
pub const CHUNK_PLAN: &str = "chunk.plan";
pub const CHUNK_CUT_OK: &str = "chunk.cut.ok";
pub const CHUNK_CUT_FAIL: &str = "chunk.cut.fail";
pub const WHISPER_START: &str = "whisper.start";
pub const WHISPER_OK: &str = "whisper.ok";
pub const WHISPER_FAIL: &str = "whisper.fail";
pub const WHISPER_OOM: &str = "whisper.oom";
pub const MERGE_OK: &str = "merge.ok";
pub const MERGE_COVERAGE_GAP: &str = "merge.coverage_gap";
pub const MERGE_BOUNDARY_RISK: &str = "merge.boundary_risk";
pub const TRANSCRIPT_EN_OK: &str = "transcript.en.ok";
pub const TRANSLATE_BLOCK_START: &str = "translate.block.start";
pub const TRANSLATE_BLOCK_OK: &str = "translate.block.ok";
pub const TRANSLATE_BLOCK_RETRY: &str = "translate.block.retry";
pub const TRANSLATE_BLOCK_FAIL: &str = "translate.block.fail";
pub const TRANSLATE_FIDELITY_WARNING: &str = "translate.fidelity_warning";
pub const TRANSCRIPT_ZH_OK: &str = "transcript.zh.ok";
pub const CLEANUP_OK: &str = "cleanup.ok";
pub const CLEANUP_SKIP: &str = "cleanup.skip";
pub const VIDEO_DONE: &str = "video.done";
pub const VIDEO_SKIP: &str = "video.skip";
pub const VIDEO_FAIL: &str = "video.fail";
pub const CANCEL_REQUESTED: &str = "cancel.requested";
pub const LOG_WRITE_FAILED: &str = "log.write_failed";
/// Raw tool output, only emitted when `logging.verbose` is enabled.
pub const TOOL_OUTPUT: &str = "tool.output";

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn event_codes_match_the_design_table_verbatim() {
    let codes = [
      RUN_START,
      RUN_END,
      PROBE_OK,
      PROBE_MISSING,
      STAGE_CHANGE,
      PLAYLIST_EXPAND,
      AUDIO_DOWNLOAD_START,
      AUDIO_DOWNLOAD_OK,
      AUDIO_DOWNLOAD_FAIL,
      DURATION_RESOLVED,
      DURATION_UNKNOWN,
      CHUNK_PLAN,
      CHUNK_CUT_OK,
      CHUNK_CUT_FAIL,
      WHISPER_START,
      WHISPER_OK,
      WHISPER_FAIL,
      WHISPER_OOM,
      MERGE_OK,
      MERGE_COVERAGE_GAP,
      MERGE_BOUNDARY_RISK,
      TRANSCRIPT_EN_OK,
      TRANSLATE_BLOCK_START,
      TRANSLATE_BLOCK_OK,
      TRANSLATE_BLOCK_RETRY,
      TRANSLATE_BLOCK_FAIL,
      TRANSLATE_FIDELITY_WARNING,
      TRANSCRIPT_ZH_OK,
      CLEANUP_OK,
      CLEANUP_SKIP,
      VIDEO_DONE,
      VIDEO_SKIP,
      VIDEO_FAIL,
      CANCEL_REQUESTED,
      LOG_WRITE_FAILED,
      TOOL_OUTPUT,
    ];
    let expected = [
      "run.start",
      "run.end",
      "probe.ok",
      "probe.missing",
      "stage.change",
      "playlist.expand",
      "audio.download.start",
      "audio.download.ok",
      "audio.download.fail",
      "duration.resolved",
      "duration.unknown",
      "chunk.plan",
      "chunk.cut.ok",
      "chunk.cut.fail",
      "whisper.start",
      "whisper.ok",
      "whisper.fail",
      "whisper.oom",
      "merge.ok",
      "merge.coverage_gap",
      "merge.boundary_risk",
      "transcript.en.ok",
      "translate.block.start",
      "translate.block.ok",
      "translate.block.retry",
      "translate.block.fail",
      "translate.fidelity_warning",
      "transcript.zh.ok",
      "cleanup.ok",
      "cleanup.skip",
      "video.done",
      "video.skip",
      "video.fail",
      "cancel.requested",
      "log.write_failed",
      "tool.output",
    ];

    assert_eq!(codes, expected);
  }
}
