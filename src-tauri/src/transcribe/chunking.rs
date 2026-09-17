//! Fixed-boundary chunk planning for the transcribe pipeline (design §3, AC-05/AC-06).
//!
//! Chunks tile `[0, duration]` on exact `chunkMinutes` boundaries. Silence alignment is
//! deliberately not performed (cut C1), so these boundaries are also the authoritative
//! cut points for `runners::ffmpeg_runner::chunk_cut_args`.

use std::fmt;

/// Upper bound for an accepted audio duration. Real recordings are far below this; the
/// bound keeps a corrupted probe value (`f64::MAX`, `inf`) from allocating an unbounded
/// plan.
const MAX_DURATION_SECONDS: f64 = 30.0 * 24.0 * 60.0 * 60.0;

const SECONDS_PER_MINUTE: f64 = 60.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkingError {
  /// Duration was zero, negative, non-finite or beyond `MAX_DURATION_SECONDS`.
  InvalidDuration,
}

impl fmt::Display for ChunkingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Self::InvalidDuration = self;
    f.write_str("audio duration must be positive and under 30 days")
  }
}

impl std::error::Error for ChunkingError {}

/// One planned cut: the half-open range `[start, end)` in seconds on the audio timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkPlan {
  pub index: usize,
  pub start: f64,
  pub end: f64,
}

/// Plans `ceil(duration / (chunk_minutes * 60))` contiguous chunks; `chunk_minutes == 0`
/// keeps a single chunk covering the whole audio (AC-05).
///
/// A chunk's `end` is the next chunk's `start`, so the union is exactly `[0, duration]`
/// with no hole and no overlap. The last chunk ends at `duration` and may be shorter than
/// `chunk_minutes`.
pub fn plan_chunks(duration: f64, chunk_minutes: u32) -> Result<Vec<ChunkPlan>, ChunkingError> {
  if !duration.is_finite() || duration <= 0.0 || duration > MAX_DURATION_SECONDS {
    return Err(ChunkingError::InvalidDuration);
  }

  if chunk_minutes == 0 {
    let single = ChunkPlan {
      index: 0,
      start: 0.0,
      end: duration,
    };
    return Ok(vec![single]);
  }

  let chunk_seconds = f64::from(chunk_minutes) * SECONDS_PER_MINUTE;
  let count = (duration / chunk_seconds).ceil() as usize;
  let mut chunks = Vec::with_capacity(count);
  for index in 0..count {
    let start = index as f64 * chunk_seconds;
    // The next chunk's `start` reuses this expression, so adjacent chunks share the exact
    // same boundary value (no hole, no overlap).
    let end = ((index + 1) as f64 * chunk_seconds).min(duration);
    chunks.push(ChunkPlan { index, start, end });
  }

  Ok(chunks)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Asserts the AC-05 tiling contract for one plan: first start is 0, last end is the
  /// duration, and every boundary is shared by exactly two chunks.
  fn assert_tiles(chunks: &[ChunkPlan], duration: f64) {
    assert!(!chunks.is_empty(), "plan must not be empty");
    let first = chunks[0];
    let last = chunks.last().copied().expect("non-empty plan");
    assert_eq!(first.start, 0.0);
    assert_eq!(last.end, duration);
    for (index, chunk) in chunks.iter().enumerate() {
      assert_eq!(chunk.index, index);
      assert!(chunk.end > chunk.start, "chunk {index} must not be empty");
    }
    for pair in chunks.windows(2) {
      assert_eq!(
        pair[0].end, pair[1].start,
        "hole or overlap at the boundary"
      );
    }
  }

  #[test]
  fn zero_minutes_keeps_a_single_chunk_covering_the_whole_audio() {
    let chunks = plan_chunks(600.0, 0).expect("plan");
    assert_eq!(
      chunks,
      vec![ChunkPlan {
        index: 0,
        start: 0.0,
        end: 600.0,
      }]
    );

    let long = plan_chunks(3601.0, 0).expect("plan");
    assert_eq!(long.len(), 1);
    assert_eq!(long[0].end, 3601.0);
  }

  #[test]
  fn fixed_boundaries_cover_3601_seconds_in_four_chunks() {
    let chunks = plan_chunks(3601.0, 20).expect("plan");
    assert_eq!(
      chunks,
      vec![
        ChunkPlan {
          index: 0,
          start: 0.0,
          end: 1200.0,
        },
        ChunkPlan {
          index: 1,
          start: 1200.0,
          end: 2400.0,
        },
        ChunkPlan {
          index: 2,
          start: 2400.0,
          end: 3600.0,
        },
        ChunkPlan {
          index: 3,
          start: 3600.0,
          end: 3601.0,
        },
      ]
    );
    assert_tiles(&chunks, 3601.0);
  }

  #[test]
  fn exact_multiples_do_not_add_an_empty_trailing_chunk() {
    let chunks = plan_chunks(2400.0, 20).expect("plan");
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[1].end, 2400.0);
    assert_tiles(&chunks, 2400.0);

    assert_eq!(plan_chunks(1200.0, 20).expect("plan").len(), 1);
  }

  #[test]
  fn one_minute_chunks_produce_the_ceil_count_and_tile_the_audio() {
    let chunks = plan_chunks(600.0, 1).expect("plan");
    assert_eq!(chunks.len(), 10);
    assert_tiles(&chunks, 600.0);

    let partial = plan_chunks(60.1, 1).expect("plan");
    assert_eq!(partial.len(), 2);
    assert_eq!(partial[1].start, 60.0);
    assert_eq!(partial[1].end, 60.1);
    assert_tiles(&partial, 60.1);
  }

  #[test]
  fn plans_tile_the_audio_for_a_table_of_durations_and_sizes() {
    for duration in [0.5, 59.9, 60.0, 60.1, 600.0, 1200.0, 3601.0, 86_400.0] {
      for minutes in [0_u32, 1, 20] {
        let chunks = plan_chunks(duration, minutes).expect("plan");
        assert_tiles(&chunks, duration);

        let expected = if minutes == 0 {
          1
        } else {
          (duration / (f64::from(minutes) * 60.0)).ceil() as usize
        };
        assert_eq!(chunks.len(), expected, "{duration}s / {minutes}min");
      }
    }
  }

  #[test]
  fn rejects_zero_negative_non_finite_and_absurd_durations() {
    for duration in [
      0.0,
      -1.0,
      f64::NAN,
      f64::INFINITY,
      f64::NEG_INFINITY,
      MAX_DURATION_SECONDS + 1.0,
      f64::MAX,
    ] {
      assert_eq!(
        plan_chunks(duration, 20),
        Err(ChunkingError::InvalidDuration),
        "duration {duration} must be rejected"
      );
      assert_eq!(
        plan_chunks(duration, 0),
        Err(ChunkingError::InvalidDuration),
        "duration {duration} must be rejected even without chunking"
      );
    }
  }

  #[test]
  fn floating_boundaries_stay_tiled_and_respect_the_ceil_count() {
    let just_over = plan_chunks(1200.0000000001, 20).expect("plan");
    assert_eq!(just_over.len(), 2);
    assert_eq!(just_over[1].start, 1200.0);
    assert_tiles(&just_over, 1200.0000000001);

    assert_eq!(plan_chunks(1199.9999999999, 20).expect("plan").len(), 1);

    let huge = plan_chunks(3601.0, u32::MAX).expect("plan");
    assert_eq!(huge.len(), 1);
    assert_eq!(huge[0].end, 3601.0);
  }

  #[test]
  fn accepts_the_longest_supported_duration() {
    let chunks = plan_chunks(MAX_DURATION_SECONDS, 0).expect("plan");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].end, MAX_DURATION_SECONDS);
  }
}
