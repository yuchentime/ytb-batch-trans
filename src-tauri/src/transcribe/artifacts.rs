//! Artifact paths, atomic writes, completeness and skip decisions (design §4/§6,
//! AC-08/AC-14).
//!
//! All deliverables are written `.tmp` → rename, so a crash cannot leave a half-written
//! `transcript.en.txt`, `segments.json` or `zh.blocks.json` that the resume logic would
//! mistake for a finished artifact.

use crate::transcribe::merge::MergedSegment;
use crate::translation::blocks::Block;
use crate::translation::deepseek_client::Usage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const WORK_DIR_NAME: &str = ".work";
pub const CHUNKS_DIR_NAME: &str = "chunks";
pub const AUDIO_STEM: &str = "audio";
pub const SEGMENTS_FILE_NAME: &str = "segments.json";
pub const ZH_BLOCKS_FILE_NAME: &str = "zh.blocks.json";
pub const EN_TRANSCRIPT_FILE_NAME: &str = "transcript.en.txt";
pub const ZH_TRANSCRIPT_FILE_NAME: &str = "transcript.zh.txt";
pub const SUMMARY_FILE_NAME: &str = "summary.md";
pub const SOURCE_FILE_NAME: &str = "source.json";

/// Longest directory name kept from a title (Windows path limits with room for `.work`).
const MAX_DIR_CHARS: usize = 120;
/// Names Windows refuses to create even with a legal character set.
const RESERVED_NAMES: [&str; 22] = [
  "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
  "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Replaces `path` with `contents` via `.tmp` + rename (AC-08). Creates parent dirs.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
  let parent = path
    .parent()
    .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
  fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;

  let tmp = path.with_extension("tmp");
  {
    let mut file =
      fs::File::create(&tmp).map_err(|error| format!("create {}: {error}", tmp.display()))?;
    file
      .write_all(contents.as_bytes())
      .map_err(|error| format!("write {}: {error}", tmp.display()))?;
    file
      .sync_all()
      .map_err(|error| format!("sync {}: {error}", tmp.display()))?;
  }
  fs::rename(&tmp, path)
    .map_err(|error| format!("rename {} -> {}: {error}", tmp.display(), path.display()))
}

/// A non-empty regular file (used for the two `.txt` deliverables).
pub fn file_is_non_empty(path: &Path) -> bool {
  fs::metadata(path)
    .map(|metadata| metadata.is_file() && metadata.len() > 0)
    .unwrap_or(false)
}

/// Where one video's artifacts live; fixed deliverable names (cut C5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPaths {
  pub dir: PathBuf,
  pub en: PathBuf,
  pub zh: PathBuf,
  pub work: PathBuf,
  pub chunks: PathBuf,
  pub segments: PathBuf,
  pub zh_blocks: PathBuf,
}

impl OutputPaths {
  pub fn new(root: &Path, title: &str, restrict_filenames: bool) -> Self {
    Self::for_dir(root.join(sanitize_directory_name(title, restrict_filenames)))
  }

  pub fn for_dir(dir: PathBuf) -> Self {
    let work = dir.join(WORK_DIR_NAME);
    Self {
      en: dir.join(EN_TRANSCRIPT_FILE_NAME),
      zh: dir.join(ZH_TRANSCRIPT_FILE_NAME),
      chunks: work.join(CHUNKS_DIR_NAME),
      segments: work.join(SEGMENTS_FILE_NAME),
      zh_blocks: work.join(ZH_BLOCKS_FILE_NAME),
      work,
      dir,
    }
  }
}

/// Builds a filesystem-safe directory name from a video title. `restrict` narrows the name
/// to ASCII/CJK alphanumerics, `-` and `_` (design's `output.restrictFilenames`).
pub fn sanitize_directory_name(title: &str, restrict: bool) -> String {
  let mut name = String::new();
  let mut last_was_fill = false;

  for ch in title.trim().chars() {
    let keep = if restrict {
      ch.is_alphanumeric() || matches!(ch, '-' | '_')
    } else {
      !(ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    };

    if keep {
      name.push(ch);
      last_was_fill = false;
    } else if !last_was_fill {
      name.push('_');
      last_was_fill = true;
    }
  }

  let mut name = name.trim_end_matches(['.', ' ']).to_string();
  if name.chars().count() > MAX_DIR_CHARS {
    name = name.chars().take(MAX_DIR_CHARS).collect();
    name = name.trim_end_matches(['.', ' ', '_']).to_string();
  }
  if name.is_empty() {
    return "untitled".to_string();
  }
  if RESERVED_NAMES
    .iter()
    .any(|reserved| name.eq_ignore_ascii_case(reserved))
  {
    name.insert(0, '_');
  }
  name
}

/// What already exists on disk, used to decide resume/skip behaviour (AC-14).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArtifactState {
  pub en_exists: bool,
  pub zh_exists: bool,
  pub segments_complete: bool,
  pub blocks_complete: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkipPlan {
  /// Both transcripts exist and `overwrite=false`: do no work at all.
  pub skip_all: bool,
  /// Reuse `segments.json` instead of re-running whisper.
  pub skip_transcription: bool,
  /// Reuse `zh.blocks.json` instead of re-querying the model.
  pub skip_translation: bool,
}

/// Decides resume behaviour (design §6). `overwrite=true` always reruns everything; a
/// reused transcription only lets the translation be reused when no new transcription
/// could change the paragraphs.
pub fn plan_skips(state: ArtifactState, overwrite: bool) -> SkipPlan {
  let skip_all = !overwrite && state.en_exists && state.zh_exists;
  let skip_transcription = skip_all || (!overwrite && state.segments_complete);
  let skip_translation = skip_transcription && (skip_all || (!overwrite && state.blocks_complete));

  SkipPlan {
    skip_all,
    skip_transcription,
    skip_translation,
  }
}

/// URL → output-directory marker kept in `.work`, so a re-run can decide "already exists"
/// without any network request (AC-14: skipping must not call yt-dlp). The directory name
/// comes from the title, which is otherwise only known after the info fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceMarker {
  pub url: String,
  pub title: String,
}

pub fn store_source_marker(work_dir: &Path, marker: &SourceMarker) -> Result<(), String> {
  let json = serde_json::to_string(marker).map_err(|error| format!("serialize source: {error}"))?;
  write_atomic(&work_dir.join(SOURCE_FILE_NAME), &json)
}

/// Finds an existing finished output for `url` (marker + both transcripts) under `root`.
/// Local filesystem only: no yt-dlp, whisper or DeepSeek call is made for a skip.
pub fn find_existing_output(root: &Path, url: &str) -> Option<OutputPaths> {
  for entry in fs::read_dir(root).ok()?.flatten() {
    let dir = entry.path();
    if !dir.is_dir() {
      continue;
    }
    let Ok(text) = fs::read_to_string(dir.join(WORK_DIR_NAME).join(SOURCE_FILE_NAME)) else {
      continue;
    };
    let Ok(marker) = serde_json::from_str::<SourceMarker>(&text) else {
      continue;
    };
    if marker.url != url {
      continue;
    }
    let outputs = OutputPaths::for_dir(dir);
    if file_is_non_empty(&outputs.en) && file_is_non_empty(&outputs.zh) {
      return Some(outputs);
    }
  }
  None
}

/// First `audio.*` file produced by the download (yt-dlp picks the container).
pub fn find_audio_file(work_dir: &Path) -> Option<PathBuf> {
  let mut candidates = fs::read_dir(work_dir)
    .ok()?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| {
      path.is_file()
        && path
          .file_stem()
          .is_some_and(|stem| stem == OsStr::new(AUDIO_STEM))
    })
    .collect::<Vec<_>>();
  candidates.sort();
  candidates.into_iter().next()
}

/// Loads `segments.json`; `None` when missing, empty or corrupt (design §6: damaged files
/// count as missing and are recomputed).
pub fn load_segments(path: &Path) -> Option<Vec<MergedSegment>> {
  let text = fs::read_to_string(path).ok()?;
  let segments = serde_json::from_str::<Vec<MergedSegment>>(&text).ok()?;
  if segments.is_empty() {
    return None;
  }
  Some(segments)
}

pub fn store_segments(path: &Path, segments: &[MergedSegment]) -> Result<(), String> {
  let json =
    serde_json::to_string(segments).map_err(|error| format!("serialize segments: {error}"))?;
  write_atomic(path, &json)
}

/// One translated block as persisted in `zh.blocks.json` (design §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockTranslation {
  pub ids: Vec<usize>,
  pub zh: Vec<String>,
  pub usage: Usage,
}

pub fn load_zh_blocks(path: &Path) -> Option<BTreeMap<usize, BlockTranslation>> {
  let text = fs::read_to_string(path).ok()?;
  serde_json::from_str::<BTreeMap<usize, BlockTranslation>>(&text).ok()
}

pub fn store_zh_blocks(
  path: &Path,
  blocks: &BTreeMap<usize, BlockTranslation>,
) -> Result<(), String> {
  let json = serde_json::to_string(blocks).map_err(|error| format!("serialize blocks: {error}"))?;
  write_atomic(path, &json)
}

/// A stored block is reusable when it covers exactly the planned ids with non-empty text.
pub fn zh_blocks_complete(blocks: &BTreeMap<usize, BlockTranslation>, plan: &[Block]) -> bool {
  plan.iter().all(|block| {
    blocks.get(&block.index).is_some_and(|stored| {
      stored.ids == block.ids()
        && stored.zh.len() == stored.ids.len()
        && stored.zh.iter().all(|text| !text.trim().is_empty())
    })
  })
}

/// Joins the stored block translations into the Chinese transcript (one paragraph per
/// line, blank line between paragraphs — same shape as the English transcript, Q6).
pub fn assemble_zh_transcript(
  plan: &[Block],
  blocks: &BTreeMap<usize, BlockTranslation>,
) -> Option<String> {
  let mut paragraphs: Vec<&str> = Vec::new();
  for block in plan {
    let stored = blocks.get(&block.index)?;
    if stored.ids != block.ids() || stored.zh.len() != stored.ids.len() {
      return None;
    }
    paragraphs.extend(stored.zh.iter().map(String::as_str));
  }
  Some(paragraphs.join("\n\n"))
}

pub fn total_usage(blocks: &BTreeMap<usize, BlockTranslation>) -> Usage {
  blocks
    .values()
    .fold(Usage::default(), |total, block| Usage {
      prompt_tokens: total.prompt_tokens + block.usage.prompt_tokens,
      completion_tokens: total.completion_tokens + block.usage.completion_tokens,
    })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::translation::blocks::BlockItem;

  fn temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ovd-artifacts-{prefix}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
  }

  fn segment(id: &str, start: f64) -> MergedSegment {
    MergedSegment {
      id: id.to_string(),
      start,
      end: start + 1.0,
      text: format!("text {id}"),
    }
  }

  fn plan_block(index: usize, ids: Vec<usize>) -> Block {
    let first = ids.first().copied().unwrap_or_default();
    let last = ids.last().copied().unwrap_or(first);
    Block {
      index,
      paragraph_range: first..last + 1,
      items: ids
        .into_iter()
        .map(|id| BlockItem {
          id,
          text: format!("paragraph {id}"),
        })
        .collect(),
    }
  }

  #[test]
  fn write_atomic_replaces_and_leaves_no_tmp_behind() {
    let dir = temp_dir("atomic");
    let target = dir.join("transcript.en.txt");

    write_atomic(&target, "first").expect("write");
    assert_eq!(fs::read_to_string(&target).expect("read"), "first");
    write_atomic(&target, "second").expect("overwrite");
    assert_eq!(fs::read_to_string(&target).expect("read"), "second");
    assert!(!dir.join("transcript.en.tmp").exists(), "tmp must be gone");
  }

  #[test]
  fn a_lingering_tmp_never_counts_as_complete() {
    let dir = temp_dir("tmp-only");
    let target = dir.join("segments.json");
    fs::write(dir.join("segments.tmp"), "[{\"id\":\"0:0\"}]").expect("write tmp");

    assert!(!file_is_non_empty(&target));
    assert!(load_segments(&target).is_none());
  }

  #[test]
  fn corrupt_or_empty_segments_count_as_missing() {
    let dir = temp_dir("segments");
    let target = dir.join("segments.json");

    assert!(load_segments(&target).is_none(), "missing");
    fs::write(&target, "{not json").expect("write");
    assert!(load_segments(&target).is_none(), "corrupt");
    fs::write(&target, "[]").expect("write");
    assert!(load_segments(&target).is_none(), "empty");

    store_segments(&target, &[segment("0:0", 0.0), segment("0:1", 1.0)]).expect("store");
    let loaded = load_segments(&target).expect("load");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[1].id, "0:1");
  }

  #[test]
  fn sanitize_directory_name_handles_illegal_reserved_and_long_titles() {
    assert_eq!(sanitize_directory_name("a/b:c*d?", false), "a_b_c_d_");
    assert_eq!(
      sanitize_directory_name("  Hello   World  ", false),
      "Hello   World"
    );
    assert_eq!(
      sanitize_directory_name("Hello, World! 2024", true),
      "Hello_World_2024"
    );
    assert_eq!(sanitize_directory_name("", false), "untitled");
    assert_eq!(sanitize_directory_name("CON", false), "_CON");
    assert_eq!(
      sanitize_directory_name(&"x".repeat(300), false)
        .chars()
        .count(),
      MAX_DIR_CHARS
    );
    assert_eq!(sanitize_directory_name("trailing... ", false), "trailing");
  }

  #[test]
  fn output_paths_use_the_fixed_deliverable_names() {
    let paths = OutputPaths::new(Path::new("/root"), "My Video", false);

    assert_eq!(paths.dir, Path::new("/root").join("My Video"));
    assert_eq!(paths.en, paths.dir.join("transcript.en.txt"));
    assert_eq!(paths.zh, paths.dir.join("transcript.zh.txt"));
    assert_eq!(paths.work, paths.dir.join(".work"));
    assert_eq!(paths.segments, paths.work.join("segments.json"));
    assert_eq!(paths.zh_blocks, paths.work.join("zh.blocks.json"));
    assert_eq!(paths.chunks, paths.work.join("chunks"));
  }

  #[test]
  fn skip_plan_covers_overwrite_existing_and_resume_cases() {
    let existing = ArtifactState {
      en_exists: true,
      zh_exists: true,
      segments_complete: true,
      blocks_complete: true,
    };
    assert_eq!(
      plan_skips(existing, false),
      SkipPlan {
        skip_all: true,
        skip_transcription: true,
        skip_translation: true,
      }
    );
    assert_eq!(plan_skips(existing, true), SkipPlan::default());

    let partial = ArtifactState {
      en_exists: false,
      zh_exists: false,
      segments_complete: true,
      blocks_complete: true,
    };
    assert_eq!(
      plan_skips(partial, false),
      SkipPlan {
        skip_all: false,
        skip_transcription: true,
        skip_translation: true,
      }
    );

    let segments_only = ArtifactState {
      segments_complete: true,
      ..ArtifactState::default()
    };
    assert_eq!(
      plan_skips(segments_only, false),
      SkipPlan {
        skip_all: false,
        skip_transcription: true,
        skip_translation: false,
      }
    );

    // A re-transcription invalidates stored translations even when blocks look complete.
    let retranscribe = ArtifactState {
      en_exists: false,
      zh_exists: false,
      segments_complete: false,
      blocks_complete: true,
    };
    assert_eq!(plan_skips(retranscribe, false), SkipPlan::default());
  }

  #[test]
  fn zh_blocks_round_trip_completeness_and_assembly() {
    let dir = temp_dir("zh-blocks");
    let target = dir.join("zh.blocks.json");
    let plan = vec![plan_block(0, vec![0, 1]), plan_block(1, vec![2])];

    let mut blocks = BTreeMap::new();
    blocks.insert(
      0,
      BlockTranslation {
        ids: vec![0, 1],
        zh: vec!["第一段。".into(), "第二段。".into()],
        usage: Usage {
          prompt_tokens: 10,
          completion_tokens: 5,
        },
      },
    );
    blocks.insert(
      1,
      BlockTranslation {
        ids: vec![2],
        zh: vec!["第三段。".into()],
        usage: Usage {
          prompt_tokens: 4,
          completion_tokens: 2,
        },
      },
    );

    assert!(zh_blocks_complete(&blocks, &plan));
    store_zh_blocks(&target, &blocks).expect("store");
    let loaded = load_zh_blocks(&target).expect("load");
    assert_eq!(loaded, blocks);
    assert_eq!(
      assemble_zh_transcript(&plan, &loaded).expect("assemble"),
      "第一段。\n\n第二段。\n\n第三段。"
    );
    assert_eq!(
      total_usage(&loaded),
      Usage {
        prompt_tokens: 14,
        completion_tokens: 7
      }
    );

    blocks.remove(&1);
    assert!(!zh_blocks_complete(&blocks, &plan));
    assert!(assemble_zh_transcript(&plan, &blocks).is_none());

    fs::write(&target, "{broken").expect("write");
    assert!(load_zh_blocks(&target).is_none());
  }

  #[test]
  fn stale_blocks_with_mismatched_ids_are_rejected() {
    let plan = vec![plan_block(0, vec![0, 1])];
    let mut blocks = BTreeMap::new();
    blocks.insert(
      0,
      BlockTranslation {
        ids: vec![1, 0],
        zh: vec!["一".into(), "二".into()],
        usage: Usage::default(),
      },
    );

    assert!(!zh_blocks_complete(&blocks, &plan));
    assert!(assemble_zh_transcript(&plan, &blocks).is_none());
  }

  #[test]
  fn source_marker_round_trips_and_finds_finished_outputs_without_network() {
    let root = temp_dir("source-marker");
    let outputs = OutputPaths::new(&root, "My Video", false);
    fs::create_dir_all(&outputs.work).expect("work dir");

    store_source_marker(
      &outputs.work,
      &SourceMarker {
        url: "https://example.com/a".into(),
        title: "My Video".into(),
      },
    )
    .expect("store marker");

    // Marker only: not a finished output yet.
    assert!(find_existing_output(&root, "https://example.com/a").is_none());

    write_atomic(&outputs.en, "english").expect("en");
    write_atomic(&outputs.zh, "中文").expect("zh");
    let found = find_existing_output(&root, "https://example.com/a").expect("found");
    assert_eq!(found.dir, outputs.dir);
    assert!(find_existing_output(&root, "https://example.com/other").is_none());
  }

  #[test]
  fn find_audio_file_ignores_partials_and_picks_a_deterministic_file() {
    let dir = temp_dir("audio");
    fs::write(dir.join("audio.m4a.part"), "partial").expect("write");
    fs::write(dir.join("audio.webm"), "audio").expect("write");
    fs::write(dir.join("audio.m4a"), "audio").expect("write");
    fs::write(dir.join("other.txt"), "other").expect("write");

    let found = find_audio_file(&dir).expect("audio file");
    assert_eq!(found.file_name().and_then(OsStr::to_str), Some("audio.m4a"));
  }
}
