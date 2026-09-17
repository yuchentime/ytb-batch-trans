//! Chunk merging for the transcribe pipeline (design §3, AC-06/AC-07).
//!
//! Chunk-local whisper segments are shifted by the chunk start, given stable
//! `{chunk_index}:{segment_index}` ids, sorted by start, and checked against the design's
//! two warning conditions: `chunkCoverageGap` (audio that no segment reaches) and
//! `chunkBoundaryRisk` (a chunk boundary that looks like a cut word). Neither check drops
//! or rewrites text: the caller keeps the scene and may rerun.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A tail that stops this close to the chunk boundary is treated as a likely cut word
/// (design §3.5).
const BOUNDARY_TAIL_SECONDS: f64 = 0.3;
/// A first segment this early suggests the previous chunk already consumed the opening
/// audio of this chunk (design §3.5).
const BOUNDARY_HEAD_SECONDS: f64 = 0.02;
/// Shortfall tolerated before a coverage gap is reported, matching the design's
/// "coverage < duration − 2s" tolerance (design §3.5).
const COVERAGE_TOLERANCE_SECONDS: f64 = 2.0;

#[derive(Debug, Clone, PartialEq)]
pub enum MergeError {
  /// `offset` was negative or non-finite.
  InvalidOffset { chunk_index: usize },
  /// `planned_end` was non-finite or before the chunk start.
  InvalidPlannedEnd { chunk_index: usize },
  /// Chunks must be given in plan order (non-decreasing offsets).
  UnsortedChunks { chunk_index: usize },
  /// A segment timestamp was non-finite, negative or reversed.
  InvalidSegment {
    chunk_index: usize,
    segment_index: usize,
  },
}

impl fmt::Display for MergeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidOffset { chunk_index } => {
        write!(f, "chunk {chunk_index}: offset must be finite and >= 0")
      }
      Self::InvalidPlannedEnd { chunk_index } => {
        write!(f, "chunk {chunk_index}: planned end must be >= start")
      }
      Self::UnsortedChunks { chunk_index } => {
        write!(f, "chunk {chunk_index}: chunks must be in plan order")
      }
      Self::InvalidSegment {
        chunk_index,
        segment_index,
      } => {
        write!(f, "segment {chunk_index}:{segment_index}: bad timestamps")
      }
    }
  }
}

impl std::error::Error for MergeError {}

/// One segment as emitted by whisper for a chunk, in chunk-local seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
  pub start: f64,
  pub end: f64,
  pub text: String,
}

/// One planned chunk's segments plus its placement on the audio timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkSegments {
  /// Chunk start (`ChunkPlan::start`); added to every segment timestamp.
  pub offset: f64,
  /// Chunk end (`ChunkPlan::end`), used for coverage and boundary checks.
  pub planned_end: f64,
  /// Segments read from `<chunk>.json`; empty when the chunk is missing or silent.
  pub segments: Vec<Segment>,
}

/// A segment after the chunk offset shift, with a stable id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergedSegment {
  /// `{chunk_index}:{segment_index}` — unique within a plan and stable across reruns.
  pub id: String,
  pub start: f64,
  pub end: f64,
  pub text: String,
}

/// Audio of one planned chunk that no segment reaches (design §3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageGap {
  /// Position in the plan passed to [`merge_segments`].
  pub chunk_index: usize,
  pub gap_seconds: f64,
}

/// A chunk boundary that looks like a cut word (design §3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryRisk {
  /// Index of the chunk that ends at this boundary.
  pub chunk_index: usize,
  /// The chunk's last segment stops within `BOUNDARY_TAIL_SECONDS` of the boundary.
  pub tail_too_close: bool,
  /// The next chunk's first segment starts no later than `BOUNDARY_HEAD_SECONDS`.
  pub head_too_early: bool,
}

/// Merge result plus the two warning lists the pipeline logs.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeOutcome {
  /// All segments, shifted and sorted by `start` (ties: `end`, then `id`).
  pub segments: Vec<MergedSegment>,
  /// Seconds covered by segments, each chunk clamped to its planned span.
  pub covered_seconds: f64,
  /// Chunks that fall short of their planned span; empty when within tolerance.
  pub coverage_gaps: Vec<CoverageGap>,
  /// Internal boundaries where a word may have been cut.
  pub boundary_risks: Vec<BoundaryRisk>,
}

/// Merges chunk-local segments onto the audio timeline (AC-07).
///
/// `chunks` must contain one entry per planned chunk, in plan order. Every segment is
/// kept: ids are derived from the position in `chunks` and the position inside the chunk,
/// so no two segments share an id and no segment is dropped. A missing chunk carries an
/// empty [`ChunkSegments::segments`] vector.
///
/// # Errors
///
/// Returns [`MergeError`] for hostile timestamps or out-of-order chunks instead of
/// silently producing a transcript with NaN times.
pub fn merge_segments(chunks: &[ChunkSegments]) -> Result<MergeOutcome, MergeError> {
  validate_chunks(chunks)?;

  let mut segments = Vec::new();
  let mut covered_seconds = 0.0;
  let mut missing_gaps = Vec::new();
  let mut partial_gaps = Vec::new();

  for (chunk_index, chunk) in chunks.iter().enumerate() {
    let span = chunk.planned_end - chunk.offset;
    let covered_local = chunk
      .segments
      .iter()
      .map(|segment| segment.end)
      .fold(0.0_f64, f64::max);
    // Validation guarantees that the span and every segment end are non-negative and
    // finite, so only the upper bound needs clamping.
    let covered = covered_local.min(span);
    covered_seconds += covered;

    let gap = span - covered;
    if chunk.segments.is_empty() {
      // A missing chunk is always reported, even when its span is tiny.
      missing_gaps.push(CoverageGap {
        chunk_index,
        gap_seconds: gap,
      });
    } else if gap > 0.0 {
      partial_gaps.push(CoverageGap {
        chunk_index,
        gap_seconds: gap,
      });
    }

    for (segment_index, segment) in chunk.segments.iter().enumerate() {
      segments.push(MergedSegment {
        id: format!("{chunk_index}:{segment_index}"),
        start: segment.start + chunk.offset,
        end: segment.end + chunk.offset,
        text: segment.text.clone(),
      });
    }
  }

  segments.sort_by(|left, right| {
    left
      .start
      .total_cmp(&right.start)
      .then_with(|| left.end.total_cmp(&right.end))
      .then_with(|| left.id.cmp(&right.id))
  });

  let mut coverage_gaps = missing_gaps;
  let total_gap = coverage_gaps
    .iter()
    .chain(partial_gaps.iter())
    .map(|gap| gap.gap_seconds)
    .sum::<f64>();
  if total_gap > COVERAGE_TOLERANCE_SECONDS {
    // Localize the shortfall so a rerun can target the chunks that lost audio.
    coverage_gaps.extend(partial_gaps);
    coverage_gaps.sort_by_key(|gap| gap.chunk_index);
  }

  Ok(MergeOutcome {
    segments,
    covered_seconds,
    coverage_gaps,
    boundary_risks: collect_boundary_risks(chunks),
  })
}

fn validate_chunks(chunks: &[ChunkSegments]) -> Result<(), MergeError> {
  let mut previous_offset = 0.0;
  for (chunk_index, chunk) in chunks.iter().enumerate() {
    if !chunk.offset.is_finite() || chunk.offset < 0.0 {
      return Err(MergeError::InvalidOffset { chunk_index });
    }
    if !chunk.planned_end.is_finite() || chunk.planned_end < chunk.offset {
      return Err(MergeError::InvalidPlannedEnd { chunk_index });
    }
    if chunk_index > 0 && chunk.offset < previous_offset {
      return Err(MergeError::UnsortedChunks { chunk_index });
    }
    previous_offset = chunk.offset;

    for (segment_index, segment) in chunk.segments.iter().enumerate() {
      let ordered = segment.start.is_finite()
        && segment.end.is_finite()
        && segment.start >= 0.0
        && segment.end >= segment.start;
      if !ordered {
        return Err(MergeError::InvalidSegment {
          chunk_index,
          segment_index,
        });
      }
    }
  }

  Ok(())
}

/// Flags internal boundaries where the tail stops just before the cut or the next chunk
/// starts right at its beginning (design §3.5, AC-06). Pre-shift chunk-local coordinates
/// are used on purpose: the check is about the cut, not about the merged timeline.
fn collect_boundary_risks(chunks: &[ChunkSegments]) -> Vec<BoundaryRisk> {
  let mut risks = Vec::new();
  for (chunk_index, pair) in chunks.windows(2).enumerate() {
    let current = &pair[0];
    let next = &pair[1];
    let span = current.planned_end - current.offset;
    let last_end = current
      .segments
      .iter()
      .map(|segment| segment.end)
      .reduce(f64::max);
    let first_start = next
      .segments
      .iter()
      .map(|segment| segment.start)
      .reduce(f64::min);

    let tail_too_close = last_end.is_some_and(|end| span - end < BOUNDARY_TAIL_SECONDS);
    let head_too_early = first_start.is_some_and(|start| start <= BOUNDARY_HEAD_SECONDS);

    if tail_too_close || head_too_early {
      risks.push(BoundaryRisk {
        chunk_index,
        tail_too_close,
        head_too_early,
      });
    }
  }

  risks
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashSet;

  fn segment(start: f64, end: f64, text: &str) -> Segment {
    Segment {
      start,
      end,
      text: text.to_string(),
    }
  }

  fn chunk(offset: f64, planned_end: f64, segments: Vec<Segment>) -> ChunkSegments {
    ChunkSegments {
      offset,
      planned_end,
      segments,
    }
  }

  /// Two chunks with a configurable tail end and head start, used by the AC-06 boundary
  /// fixtures (target boundary 600s). Segment times are chunk-local.
  fn boundary_fixture(first_end: f64, second_start: f64) -> MergeOutcome {
    merge_segments(&[
      chunk(0.0, 600.0, vec![segment(0.0, first_end, "tail")]),
      chunk(600.0, 1200.0, vec![segment(second_start, 599.5, "head")]),
    ])
    .expect("merge")
  }

  #[test]
  fn shifts_sorts_and_preserves_every_segment() {
    let chunks = [
      chunk(
        0.0,
        600.0,
        vec![segment(10.0, 12.0, "one"), segment(0.0, 599.5, "zero")],
      ),
      chunk(600.0, 1200.0, vec![segment(0.5, 599.5, "two")]),
      chunk(
        1200.0,
        1800.0,
        vec![segment(3.0, 4.0, "three"), segment(1.0, 599.5, "two-b")],
      ),
    ];

    let outcome = merge_segments(&chunks).expect("merge");

    assert_eq!(outcome.segments.len(), 5, "no segment may be dropped");
    let starts = outcome
      .segments
      .iter()
      .map(|segment| segment.start)
      .collect::<Vec<_>>();
    assert_eq!(starts, vec![0.0, 10.0, 600.5, 1201.0, 1203.0]);
    assert!(outcome
      .segments
      .windows(2)
      .all(|pair| pair[0].start <= pair[1].start));
    let texts = outcome
      .segments
      .iter()
      .map(|segment| segment.text.as_str())
      .collect::<Vec<_>>();
    assert_eq!(texts, vec!["zero", "one", "two", "two-b", "three"]);
    let ids = outcome
      .segments
      .iter()
      .map(|segment| segment.id.as_str())
      .collect::<HashSet<_>>();
    assert_eq!(ids.len(), 5, "ids must be unique");
    assert_eq!(outcome.segments[2].id, "1:0");
    assert_eq!(outcome.covered_seconds, 1798.5);
    assert!(outcome.coverage_gaps.is_empty());
    assert!(outcome.boundary_risks.is_empty());
  }

  #[test]
  fn identical_timestamps_from_different_chunks_stay_separate() {
    let chunks = [
      chunk(0.0, 600.0, vec![segment(1.0, 2.0, "a")]),
      chunk(600.0, 1200.0, vec![segment(1.0, 2.0, "b")]),
    ];

    let outcome = merge_segments(&chunks).expect("merge");

    assert_eq!(outcome.segments.len(), 2);
    assert_eq!(outcome.segments[0].id, "0:0");
    assert_eq!(outcome.segments[1].id, "1:0");
    assert_eq!(outcome.segments[0].start, 1.0);
    assert_eq!(outcome.segments[1].start, 601.0);
  }

  #[test]
  fn flags_a_tail_that_stops_within_the_threshold() {
    let risk = boundary_fixture(599.9, 1.2);
    assert_eq!(risk.boundary_risks.len(), 1);
    assert_eq!(risk.boundary_risks[0].chunk_index, 0);
    assert!(risk.boundary_risks[0].tail_too_close);
    assert!(!risk.boundary_risks[0].head_too_early);

    let safe = boundary_fixture(598.0, 1.2);
    assert!(safe.boundary_risks.is_empty());
  }

  #[test]
  fn the_tail_threshold_is_strict() {
    // 0.25s from the boundary is below the 0.3s threshold; 0.3s exactly (0.5 - 0.2, both
    // exact in binary floating point) is not.
    let below = merge_segments(&[
      chunk(0.0, 0.5, vec![segment(0.0, 0.25, "tail")]),
      chunk(0.5, 1.0, vec![segment(0.1, 0.9, "head")]),
    ])
    .expect("merge");
    assert_eq!(below.boundary_risks.len(), 1);
    assert!(below.boundary_risks[0].tail_too_close);

    let at_threshold = merge_segments(&[
      chunk(0.0, 0.5, vec![segment(0.0, 0.2, "tail")]),
      chunk(0.5, 1.0, vec![segment(0.1, 0.9, "head")]),
    ])
    .expect("merge");
    assert!(at_threshold.boundary_risks.is_empty());
  }

  #[test]
  fn flags_a_head_that_starts_at_the_very_beginning_of_a_chunk() {
    let threshold = boundary_fixture(598.0, 0.02);
    assert_eq!(threshold.boundary_risks.len(), 1);
    assert!(threshold.boundary_risks[0].head_too_early);
    assert!(!threshold.boundary_risks[0].tail_too_close);

    let zero = boundary_fixture(598.0, 0.0);
    assert!(zero.boundary_risks[0].head_too_early);

    assert!(boundary_fixture(598.0, 0.021).boundary_risks.is_empty());
    assert!(boundary_fixture(598.0, 1.2).boundary_risks.is_empty());
  }

  #[test]
  fn a_boundary_can_report_both_conditions() {
    let both = boundary_fixture(599.9, 0.01);
    assert_eq!(both.boundary_risks.len(), 1);
    assert!(both.boundary_risks[0].tail_too_close);
    assert!(both.boundary_risks[0].head_too_early);
  }

  #[test]
  fn a_missing_chunk_is_always_reported_and_localizes_the_shortfall() {
    let outcome = merge_segments(&[
      chunk(0.0, 600.0, vec![segment(0.0, 599.5, "ok")]),
      chunk(600.0, 1200.0, Vec::new()),
    ])
    .expect("merge");

    // The missing chunk is 600s short, so the total exceeds the 2s tolerance and every
    // chunk that lost audio is listed (chunk 0's 0.5s tail included).
    assert_eq!(
      outcome.coverage_gaps,
      vec![
        CoverageGap {
          chunk_index: 0,
          gap_seconds: 0.5,
        },
        CoverageGap {
          chunk_index: 1,
          gap_seconds: 600.0,
        },
      ]
    );
    assert_eq!(outcome.covered_seconds, 599.5);
  }

  #[test]
  fn ignores_small_tail_losses_below_the_tolerance() {
    let outcome = merge_segments(&[
      chunk(0.0, 600.0, vec![segment(0.0, 599.0, "a")]),
      chunk(600.0, 1200.0, vec![segment(0.5, 599.5, "b")]),
    ])
    .expect("merge");

    assert!(outcome.coverage_gaps.is_empty());
    assert_eq!(outcome.covered_seconds, 599.0 + 599.5);
  }

  #[test]
  fn reports_a_single_chunk_that_falls_short_by_more_than_the_tolerance() {
    let outcome =
      merge_segments(&[chunk(0.0, 600.0, vec![segment(0.0, 597.5, "a")])]).expect("merge");

    assert_eq!(
      outcome.coverage_gaps,
      vec![CoverageGap {
        chunk_index: 0,
        gap_seconds: 2.5,
      }]
    );
    assert_eq!(outcome.covered_seconds, 597.5);
  }

  #[test]
  fn small_losses_accumulate_into_a_reported_gap() {
    let outcome = merge_segments(&[
      chunk(0.0, 600.0, vec![segment(0.0, 598.0, "a")]),
      chunk(600.0, 1200.0, vec![segment(0.5, 598.5, "b")]),
    ])
    .expect("merge");

    assert_eq!(outcome.coverage_gaps.len(), 2);
    assert_eq!(outcome.coverage_gaps[0].chunk_index, 0);
    assert_eq!(outcome.coverage_gaps[0].gap_seconds, 2.0);
    assert_eq!(outcome.coverage_gaps[1].chunk_index, 1);
    assert_eq!(outcome.coverage_gaps[1].gap_seconds, 1.5);
    assert_eq!(outcome.covered_seconds, 598.0 + 598.5);
  }

  #[test]
  fn rejects_hostile_offsets_and_planned_ends() {
    assert_eq!(
      merge_segments(&[chunk(-1.0, 600.0, Vec::new())]),
      Err(MergeError::InvalidOffset { chunk_index: 0 })
    );
    assert_eq!(
      merge_segments(&[chunk(f64::NAN, 600.0, Vec::new())]),
      Err(MergeError::InvalidOffset { chunk_index: 0 })
    );
    assert_eq!(
      merge_segments(&[chunk(600.0, 599.0, Vec::new())]),
      Err(MergeError::InvalidPlannedEnd { chunk_index: 0 })
    );
    assert_eq!(
      merge_segments(&[chunk(0.0, f64::INFINITY, Vec::new())]),
      Err(MergeError::InvalidPlannedEnd { chunk_index: 0 })
    );
  }

  #[test]
  fn rejects_out_of_order_chunks() {
    let chunks = [
      chunk(600.0, 1200.0, Vec::new()),
      chunk(0.0, 600.0, Vec::new()),
    ];

    assert_eq!(
      merge_segments(&chunks),
      Err(MergeError::UnsortedChunks { chunk_index: 1 })
    );
  }

  #[test]
  fn rejects_hostile_segment_timestamps() {
    let cases = [
      segment(f64::NAN, 1.0, "nan start"),
      segment(0.0, f64::INFINITY, "inf end"),
      segment(-0.1, 1.0, "negative start"),
      segment(2.0, 1.0, "reversed"),
    ];

    for case in cases {
      let chunks = [chunk(0.0, 600.0, vec![case])];
      assert_eq!(
        merge_segments(&chunks),
        Err(MergeError::InvalidSegment {
          chunk_index: 0,
          segment_index: 0,
        })
      );
    }
  }

  #[test]
  fn empty_input_is_a_valid_empty_merge() {
    let outcome = merge_segments(&[]).expect("merge");

    assert!(outcome.segments.is_empty());
    assert_eq!(outcome.covered_seconds, 0.0);
    assert!(outcome.coverage_gaps.is_empty());
    assert!(outcome.boundary_risks.is_empty());
  }

  #[test]
  fn a_segment_running_past_its_chunk_span_is_clamped_never_double_counted() {
    let outcome = merge_segments(&[
      chunk(0.0, 600.0, vec![segment(0.0, 610.0, "overrun")]),
      chunk(600.0, 1200.0, vec![segment(1.2, 599.5, "next")]),
    ])
    .expect("merge");

    assert_eq!(outcome.covered_seconds, 600.0 + 599.5);
    assert!(outcome.coverage_gaps.is_empty());
    assert_eq!(outcome.boundary_risks.len(), 1);
    assert!(outcome.boundary_risks[0].tail_too_close);
  }
}
