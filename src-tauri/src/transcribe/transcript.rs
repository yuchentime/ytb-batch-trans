//! Mechanical English transcript formatting (design §4, AC-09).
//!
//! This module only decides where blank lines go. It never rewrites, drops or reorders a
//! word, and it deliberately does not depend on `translation::*`: the English transcript
//! must stay independent of any LLM path (G2 guardrail).

use crate::transcribe::merge::MergedSegment;

/// A silence longer than this starts a new paragraph (design §4).
const PARAGRAPH_GAP_SECONDS: f64 = 1.2;
/// A paragraph closes once its rendered length reaches this many characters.
const PARAGRAPH_MAX_CHARS: usize = 700;
/// A paragraph closes once this many of its segments end with sentence punctuation.
const PARAGRAPH_SENTENCE_LIMIT: usize = 3;
/// Whisper's English output marks sentence ends with these characters. Counting segments
/// that *end* with one of them (instead of every character) keeps `...` and decimals like
/// `3.5` from inflating the sentence count.
const SENTENCE_END_CHARS: [char; 4] = ['.', '!', '?', '…'];

/// The mechanical paragraphing result; `text` is `paragraphs` joined with a blank line.
#[derive(Debug, Clone, PartialEq)]
pub struct EnglishTranscript {
  /// Paragraphs in order; each is its segments joined with a single space.
  pub paragraphs: Vec<String>,
  /// The deliverable text, without a trailing newline.
  pub text: String,
}

impl EnglishTranscript {
  pub fn paragraph_count(&self) -> usize {
    self.paragraphs.len()
  }

  /// Unicode scalar count, matching the `chars` field of the `transcript.en.ok` event.
  pub fn char_count(&self) -> usize {
    self.text.chars().count()
  }
}

/// Formats segments into paragraphs without touching a single word (design §4, AC-09).
///
/// A new paragraph starts before a segment when, in this order:
/// 1. the silence since the previous segment exceeds `PARAGRAPH_GAP_SECONDS`;
/// 2. the current paragraph already reached `PARAGRAPH_MAX_CHARS` rendered characters;
/// 3. the current paragraph ends at least three sentence-final segments ago and its last
///    segment ends with sentence punctuation.
///
/// Whitespace-only segments carry no transcript and are skipped, so gaps are measured
/// from the last non-empty segment.
pub fn format_english_transcript(segments: &[MergedSegment]) -> EnglishTranscript {
  let mut paragraphs: Vec<String> = Vec::new();
  let mut current: Vec<&str> = Vec::new();
  let mut current_chars = 0_usize;
  let mut current_sentence_ends = 0_usize;
  let mut previous_end: Option<f64> = None;
  let mut last_ends_sentence = false;

  for segment in segments {
    let text = segment.text.trim();
    if text.is_empty() {
      continue;
    }

    let starts_paragraph = previous_end.is_some_and(|end| {
      let gap = segment.start - end;
      gap > PARAGRAPH_GAP_SECONDS
        || current_chars >= PARAGRAPH_MAX_CHARS
        || (current_sentence_ends >= PARAGRAPH_SENTENCE_LIMIT && last_ends_sentence)
    });

    if starts_paragraph && !current.is_empty() {
      paragraphs.push(current.join(" "));
      current.clear();
      current_chars = 0;
      current_sentence_ends = 0;
    }

    if !current.is_empty() {
      current_chars += 1; // the joining space
    }
    current_chars += text.chars().count();
    let ends_sentence = ends_with_sentence_punctuation(text);
    if ends_sentence {
      current_sentence_ends += 1;
    }
    last_ends_sentence = ends_sentence;
    current.push(text);
    previous_end = Some(segment.end);
  }

  if !current.is_empty() {
    paragraphs.push(current.join(" "));
  }

  let text = paragraphs.join("\n\n");

  EnglishTranscript { paragraphs, text }
}

fn ends_with_sentence_punctuation(text: &str) -> bool {
  text.ends_with(&SENTENCE_END_CHARS[..])
}

#[cfg(test)]
mod tests {
  use super::*;

  fn text_segment(start: f64, end: f64, text: &str) -> MergedSegment {
    MergedSegment {
      id: String::new(),
      start,
      end,
      text: text.to_string(),
    }
  }

  #[test]
  fn keeps_a_paragraph_across_a_gap_of_exactly_1_2_seconds() {
    // The first segment ends at 0.0 so the subtraction is exact.
    let transcript = format_english_transcript(&[
      text_segment(0.0, 0.0, "First."),
      text_segment(PARAGRAPH_GAP_SECONDS, 2.0, "Second."),
    ]);

    assert_eq!(transcript.paragraphs, vec!["First. Second."]);
  }

  #[test]
  fn starts_a_new_paragraph_when_the_gap_exceeds_the_threshold() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 0.0, "First."),
      text_segment(PARAGRAPH_GAP_SECONDS + 0.001, 2.0, "Second."),
    ]);

    assert_eq!(transcript.paragraphs, vec!["First.", "Second."]);
    assert_eq!(transcript.text, "First.\n\nSecond.");
  }

  #[test]
  fn closes_a_paragraph_once_it_reaches_seven_hundred_rendered_characters() {
    let long = "a".repeat(350);
    let medium = "b".repeat(348);
    let full = "b".repeat(349);
    let tail = "c".repeat(10);

    // 350 + 1 + 348 = 699 rendered characters, so the third segment still fits.
    let fits = format_english_transcript(&[
      text_segment(0.0, 1.0, &long),
      text_segment(1.1, 2.0, &medium),
      text_segment(2.1, 3.0, &tail),
    ]);
    assert_eq!(fits.paragraph_count(), 1);

    // 350 + 1 + 349 = 700, so the third segment starts a new paragraph.
    let split = format_english_transcript(&[
      text_segment(0.0, 1.0, &long),
      text_segment(1.1, 2.0, &full),
      text_segment(2.1, 3.0, &tail),
    ]);
    assert_eq!(split.paragraph_count(), 2);
    assert_eq!(split.paragraphs[1], tail);
    assert_eq!(split.char_count(), 350 + 1 + 349 + 2 + 10);
  }

  #[test]
  fn closes_a_paragraph_after_three_sentence_final_segments() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, "One."),
      text_segment(1.1, 2.0, "Two."),
      text_segment(2.1, 3.0, "Three."),
      text_segment(3.1, 4.0, "Four."),
    ]);

    assert_eq!(transcript.paragraphs, vec!["One. Two. Three.", "Four."]);
  }

  #[test]
  fn keeps_a_paragraph_that_has_only_two_sentence_ends() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, "One."),
      text_segment(1.1, 2.0, "Two."),
      text_segment(2.1, 3.0, "and more"),
    ]);

    assert_eq!(transcript.paragraph_count(), 1);
  }

  #[test]
  fn a_continuation_after_three_sentence_ends_opens_the_next_paragraph() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, "One."),
      text_segment(1.1, 2.0, "Two."),
      text_segment(2.1, 3.0, "Three."),
      text_segment(3.1, 4.0, "and more"),
      text_segment(4.1, 5.0, "Four."),
      text_segment(5.1, 6.0, "Five."),
    ]);

    // The break happens before the segment that follows `Three.`, so the continuation
    // opens the next paragraph rather than closing the previous one.
    assert_eq!(
      transcript.paragraphs,
      vec!["One. Two. Three.", "and more Four. Five."]
    );
  }
  #[test]
  fn only_the_final_character_decides_a_sentence_end() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, "One."),
      text_segment(1.1, 2.0, "Two."),
      text_segment(2.1, 3.0, "Three.\""),
      text_segment(3.1, 4.0, "Four."),
      text_segment(4.1, 5.0, "Five."),
    ]);

    // `Three."` does not end with sentence punctuation, so that paragraph only closes
    // once `Four.` provides a sentence-final tail; `Five.` then starts the next one.
    assert_eq!(
      transcript.paragraphs,
      vec!["One. Two. Three.\" Four.", "Five."]
    );
  }

  #[test]
  fn keeps_segment_text_verbatim() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, "  Hello   world . "),
      text_segment(1.1, 2.0, "Second  part"),
    ]);

    assert_eq!(transcript.text, "Hello   world . Second  part");
  }

  #[test]
  fn skips_empty_segments_and_measures_gaps_from_the_last_non_empty_one() {
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, "First."),
      text_segment(2.0, 2.5, "   "),
      text_segment(2.5, 3.0, ""),
      text_segment(3.0, 4.0, "Second."),
    ]);

    // The gap is 2.0s against the last non-empty segment, so the two sentences do not
    // end up in one paragraph.
    assert_eq!(transcript.paragraphs, vec!["First.", "Second."]);
  }

  #[test]
  fn counts_unicode_scalars_and_keeps_text_consistent_with_paragraphs() {
    let long = "汉".repeat(PARAGRAPH_MAX_CHARS);
    let transcript = format_english_transcript(&[
      text_segment(0.0, 1.0, &long),
      text_segment(1.1, 2.0, "next"),
    ]);

    assert_eq!(transcript.paragraph_count(), 2);
    assert_eq!(transcript.char_count(), PARAGRAPH_MAX_CHARS + 2 + 4);
    assert_eq!(transcript.text, transcript.paragraphs.join("\n\n"));
  }

  #[test]
  fn an_empty_transcript_stays_empty() {
    let transcript = format_english_transcript(&[]);

    assert!(transcript.paragraphs.is_empty());
    assert_eq!(transcript.text, "");
    assert_eq!(transcript.paragraph_count(), 0);
    assert_eq!(transcript.char_count(), 0);
  }

  #[test]
  fn emoji_are_single_scalars_and_overlapping_segments_never_break() {
    let emoji = format_english_transcript(&[
      text_segment(0.0, 1.0, &"😀".repeat(PARAGRAPH_MAX_CHARS)),
      text_segment(1.1, 2.0, "next"),
    ]);
    assert_eq!(emoji.paragraph_count(), 2);

    let overlap = format_english_transcript(&[
      text_segment(0.0, 5.0, "First."),
      text_segment(2.0, 6.0, "Second."),
    ]);
    assert_eq!(overlap.paragraph_count(), 1);
  }
}
