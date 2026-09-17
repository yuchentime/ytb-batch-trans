//! Paragraph → block planning and prompt assembly for the DeepSeek translation (design §5).
//!
//! Paragraphs are never split: the Chinese transcript must stay 1:1 with the English one
//! (Q6), so a block is a consecutive run of paragraphs bounded by
//! `max_segments_per_block` and `max_chars_per_block`. A paragraph longer than the char
//! budget still gets its own block. All prompt text lives here so that a reviewer can read
//! the exact instructions in one place.

use crate::state::config_models::TranslationSettings;
use std::ops::Range;

/// Number of already translated paragraphs sent as context before a block (design §5).
pub const CONTEXT_PARAGRAPHS: usize = 2;

/// One paragraph inside a block; the global paragraph index doubles as the response id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockItem {
  pub id: usize,
  pub text: String,
}

/// A consecutive run of paragraphs sent to the model in one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
  pub index: usize,
  /// Global paragraph range covered by this block (half-open).
  pub paragraph_range: Range<usize>,
  pub items: Vec<BlockItem>,
}

impl Block {
  pub fn ids(&self) -> Vec<usize> {
    self.items.iter().map(|item| item.id).collect()
  }

  pub fn chars(&self) -> usize {
    self
      .items
      .iter()
      .map(|item| item.text.chars().count())
      .sum()
  }
}

/// One previous paragraph with its finished translation, sent as context only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextParagraph {
  pub source: String,
  pub translation: String,
}

/// Up to [`CONTEXT_PARAGRAPHS`] paragraphs before the current block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockContext {
  pub previous: Vec<ContextParagraph>,
}

impl BlockContext {
  /// Builds the context for `block` from the full paragraph list and the translations
  /// produced so far (indexed by paragraph id, as in `zh.blocks.json`).
  pub fn before(block: &Block, paragraphs: &[String], translations: &[String]) -> Self {
    let end = block.paragraph_range.start;
    let start = end.saturating_sub(CONTEXT_PARAGRAPHS);
    let previous = (start..end)
      .filter_map(|id| {
        Some(ContextParagraph {
          source: paragraphs.get(id)?.clone(),
          translation: translations.get(id)?.clone(),
        })
      })
      .collect();
    Self { previous }
  }
}

/// Splits transcript paragraphs into blocks; `0` limits are clamped to `1` so a
/// misconfigured setting can never produce an empty or infinitely growing block.
pub fn plan_blocks(paragraphs: &[String], settings: &TranslationSettings) -> Vec<Block> {
  let max_items = settings.max_segments_per_block.max(1) as usize;
  let max_chars = settings.max_chars_per_block.max(1) as usize;

  let mut blocks: Vec<Block> = Vec::new();
  let mut items: Vec<BlockItem> = Vec::new();
  let mut chars = 0_usize;

  for (id, text) in paragraphs.iter().enumerate() {
    let text_chars = text.chars().count();
    let overflows =
      !items.is_empty() && (items.len() >= max_items || chars + text_chars > max_chars);
    if overflows {
      blocks.push(finish_block(blocks.len(), &mut items, &mut chars));
    }
    items.push(BlockItem {
      id,
      text: text.clone(),
    });
    chars += text_chars;
  }

  if !items.is_empty() {
    blocks.push(finish_block(blocks.len(), &mut items, &mut chars));
  }

  blocks
}

fn finish_block(index: usize, items: &mut Vec<BlockItem>, chars: &mut usize) -> Block {
  let first = items.first().map(|item| item.id).unwrap_or_default();
  let last = items.last().map(|item| item.id).unwrap_or(first);
  let block = Block {
    index,
    paragraph_range: first..last + 1,
    items: std::mem::take(items),
  };
  *chars = 0;
  block
}

/// Parses the plain-text glossary (one `source=target` pair per line, cut C6). Blank
/// lines and lines without a non-empty pair are ignored.
pub fn parse_glossary(glossary: &str) -> Vec<(String, String)> {
  glossary
    .lines()
    .filter_map(|line| {
      let (source, target) = line.split_once('=')?;
      let source = source.trim();
      let target = target.trim();
      if source.is_empty() || target.is_empty() {
        return None;
      }
      Some((source.to_string(), target.to_string()))
    })
    .collect()
}

const ROLE: &str = "You are a professional translator. You translate English video transcripts into Simplified Chinese (zh-Hans) for a readable written transcript.";
const HARD_RULES: &str = "\nHard rules:\n\
- Preserve the meaning exactly: never add, omit, summarise, explain or annotate.\n\
- Do not add titles, headings, notes, markdown or any commentary.\n\
- Keep numbers, dates, times, URLs, code and proper names verbatim.\n\
- Keep exactly one Chinese paragraph per input paragraph, in the same id order.";
const FILLER_RULE: &str = "\n- Remove filler words (um, uh, you know, like, sort of) and clearly garbled phrases; make no other change.";
const OUTPUT_CONTRACT: &str = "\nOutput contract: reply with a single JSON object of the form {\"translations\":[{\"id\":<id>,\"zh\":\"<Chinese paragraph>\"}]}. Include every id in the input order, add no extra keys, and write no text outside the JSON.";

/// Builds the system prompt: role, hard rules, optional filler rule, glossary and the
/// JSON output contract (design §5).
pub fn build_system_prompt(settings: &TranslationSettings) -> String {
  let mut prompt = String::from(ROLE);
  prompt.push_str(HARD_RULES);
  if settings.drop_fillers {
    prompt.push_str(FILLER_RULE);
  }

  let glossary = parse_glossary(&settings.glossary);
  if !glossary.is_empty() {
    prompt.push_str("\nGlossary (use the target term whenever its source term appears):\n");
    for (source, target) in glossary {
      prompt.push_str(&format!("- {source} = {target}\n"));
    }
  }

  prompt.push_str(OUTPUT_CONTRACT);
  prompt
}

/// Builds the user prompt: context paragraphs (not to be re-translated) plus the current
/// block as `[id] text` lines.
pub fn build_user_prompt(block: &Block, context: &BlockContext) -> String {
  let mut prompt = String::new();

  if !context.previous.is_empty() {
    prompt.push_str("Already translated context (use it only as reference, do not repeat it):\n");
    for paragraph in &context.previous {
      prompt.push_str("- source: ");
      prompt.push_str(&paragraph.source);
      prompt.push('\n');
      prompt.push_str("  translation: ");
      prompt.push_str(&paragraph.translation);
      prompt.push('\n');
    }
    prompt.push('\n');
  }

  prompt.push_str(
    "Translate every paragraph below into Simplified Chinese. \
     Return the JSON object described in the system prompt, one translation per id.\n\n",
  );
  for item in &block.items {
    prompt.push_str(&format!("[{}] {}\n", item.id, item.text));
  }
  prompt
}

#[cfg(test)]
mod tests {
  use super::*;

  fn paragraphs(count: usize, chars: usize) -> Vec<String> {
    (0..count)
      .map(|index| format!("{}{index}", "x".repeat(chars.saturating_sub(1))))
      .collect()
  }

  fn settings(max_segments: u32, max_chars: u32) -> TranslationSettings {
    TranslationSettings {
      max_segments_per_block: max_segments,
      max_chars_per_block: max_chars,
      ..TranslationSettings::default()
    }
  }

  #[test]
  fn splits_on_the_segment_limit() {
    let blocks = plan_blocks(&paragraphs(7, 10), &settings(3, 10_000));

    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].ids(), vec![0, 1, 2]);
    assert_eq!(blocks[1].ids(), vec![3, 4, 5]);
    assert_eq!(blocks[2].ids(), vec![6]);
    assert_eq!(blocks[1].paragraph_range, 3..6);
    assert_eq!(blocks[1].index, 1);
  }

  #[test]
  fn splits_on_the_character_limit() {
    // Four 10-char paragraphs, 25-char budget: two per block.
    let blocks = plan_blocks(&paragraphs(4, 10), &settings(100, 25));

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].ids(), vec![0, 1]);
    assert_eq!(blocks[1].ids(), vec![2, 3]);
  }

  #[test]
  fn a_single_oversized_paragraph_still_gets_its_own_block() {
    let source = vec!["x".repeat(5_000), "short".into()];
    let blocks = plan_blocks(&source, &settings(6, 3_000));

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].ids(), vec![0]);
    assert_eq!(blocks[0].chars(), 5_000);
    assert_eq!(blocks[1].ids(), vec![1]);
  }

  #[test]
  fn zero_limits_never_create_empty_or_unbounded_blocks() {
    let blocks = plan_blocks(&paragraphs(3, 4), &settings(0, 0));

    assert_eq!(blocks.len(), 3);
    for (index, block) in blocks.iter().enumerate() {
      assert_eq!(block.ids(), vec![index]);
    }
  }

  #[test]
  fn an_empty_transcript_has_no_blocks() {
    assert!(plan_blocks(&[], &settings(6, 3_000)).is_empty());
  }

  #[test]
  fn glossary_parsing_skips_malformed_lines_and_keeps_the_first_equals() {
    let glossary = parse_glossary(
      "  neural network = 神经网络 \n\nbroken line\n=empty source\nempty target=\nAPI=a=b\n",
    );

    assert_eq!(
      glossary,
      vec![
        ("neural network".to_string(), "神经网络".to_string()),
        ("API".to_string(), "a=b".to_string()),
      ]
    );
  }

  #[test]
  fn system_prompt_carries_rules_glossary_and_json_contract() {
    let mut settings = settings(6, 3_000);
    settings.glossary = "neural network=神经网络".into();
    let prompt = build_system_prompt(&settings);

    assert!(prompt.contains("Simplified Chinese"));
    assert!(prompt.contains("never add, omit, summarise"));
    assert!(prompt.contains("Remove filler words"));
    assert!(prompt.contains("neural network = 神经网络"));
    assert!(prompt.contains("\"translations\""));
  }

  #[test]
  fn drop_fillers_toggle_changes_the_prompt() {
    let mut settings = settings(6, 3_000);
    settings.drop_fillers = false;

    assert!(!build_system_prompt(&settings).contains("Remove filler words"));
  }

  #[test]
  fn user_prompt_lists_ids_and_the_previous_context() {
    let paragraphs = vec![
      "First.".to_string(),
      "Second.".to_string(),
      "Third.".to_string(),
    ];
    let translations = vec!["第一。".to_string(), "第二。".to_string()];
    let blocks = plan_blocks(&paragraphs, &settings(1, 100));
    let context = BlockContext::before(&blocks[2], &paragraphs, &translations);
    let prompt = build_user_prompt(&blocks[2], &context);

    assert_eq!(context.previous.len(), 2);
    assert_eq!(context.previous[0].source, "First.");
    assert_eq!(context.previous[1].translation, "第二。");
    assert!(prompt.contains("[2] Third."));
    assert!(prompt.contains("source: First."));
    assert!(prompt.contains("translation: 第二。"));
  }

  #[test]
  fn context_before_the_first_block_is_empty() {
    let paragraphs = vec!["First.".to_string()];
    let blocks = plan_blocks(&paragraphs, &settings(6, 3_000));
    let context = BlockContext::before(&blocks[0], &paragraphs, &[]);

    assert!(context.previous.is_empty());
  }
}
