//! Response contract and numeric-fidelity checks for block translation (design §5,
//! AC-10/AC-11).
//!
//! A block that fails the contract is never persisted: the caller retries and, when the
//! retries are exhausted, fails the video with `translationContractViolation`. Numeric
//! fidelity is advisory only (`translationFidelityWarning`) and must not fail a block.

use serde::{Deserialize, Serialize};
use std::fmt;

/// One translated paragraph as returned by the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationItem {
  pub id: usize,
  pub zh: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
  MissingId(usize),
  UnexpectedId(usize),
  OutOfOrder { expected: usize, found: usize },
  EmptyTranslation(usize),
  WrongItemCount { expected: usize, found: usize },
}

impl fmt::Display for ValidationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::MissingId(id) => write!(f, "missing translation for id {id}"),
      Self::UnexpectedId(id) => write!(f, "unexpected translation id {id}"),
      Self::OutOfOrder { expected, found } => {
        write!(f, "expected id {expected} at this position, found {found}")
      }
      Self::EmptyTranslation(id) => write!(f, "empty translation for id {id}"),
      Self::WrongItemCount { expected, found } => {
        write!(f, "expected {expected} translations, found {found}")
      }
    }
  }
}

impl std::error::Error for ValidationError {}

/// Validates one model response against the requested ids (AC-10): the id set must match,
/// the order must match, and every `zh` must be non-empty.
pub fn validate_block_response(
  expected_ids: &[usize],
  response: &[TranslationItem],
) -> Result<(), ValidationError> {
  for expected in expected_ids {
    if !response.iter().any(|item| item.id == *expected) {
      return Err(ValidationError::MissingId(*expected));
    }
  }
  for item in response {
    if !expected_ids.contains(&item.id) {
      return Err(ValidationError::UnexpectedId(item.id));
    }
  }
  if response.len() != expected_ids.len() {
    return Err(ValidationError::WrongItemCount {
      expected: expected_ids.len(),
      found: response.len(),
    });
  }

  for (expected, item) in expected_ids.iter().zip(response) {
    if item.id != *expected {
      return Err(ValidationError::OutOfOrder {
        expected: *expected,
        found: item.id,
      });
    }
    if item.zh.trim().is_empty() {
      return Err(ValidationError::EmptyTranslation(item.id));
    }
  }

  Ok(())
}

/// Digit runs in `source`, keeping `.`/`,` separators between digits and a trailing `%`
/// (e.g. `2,024`, `3.5`, `90%`); duplicates are removed, order is preserved.
pub fn extract_numbers(source: &str) -> Vec<String> {
  let chars = source.chars().collect::<Vec<_>>();
  let mut numbers: Vec<String> = Vec::new();
  let mut index = 0;

  while index < chars.len() {
    if !chars[index].is_ascii_digit() {
      index += 1;
      continue;
    }

    let start = index;
    while index < chars.len() {
      if chars[index].is_ascii_digit() {
        index += 1;
        continue;
      }
      let separator = matches!(chars[index], '.' | ',')
        && chars
          .get(index + 1)
          .is_some_and(|next| next.is_ascii_digit());
      if separator {
        index += 1;
        continue;
      }
      break;
    }
    if matches!(chars.get(index), Some('%' | '％')) {
      index += 1;
    }

    let token = chars[start..index]
      .iter()
      .collect::<String>()
      .replace('％', "%");
    if !numbers.contains(&token) {
      numbers.push(token);
    }
  }

  numbers
}

/// Returns the numbers of `source` that are missing from `translation` (AC-11). A number
/// counts as present when it appears with number boundaries in the normalized translation,
/// or when the glossary maps a source term containing it to a target that the translation
/// used. The result is advisory: callers log `translationFidelityWarning` and continue.
pub fn check_numeric_fidelity(
  source: &str,
  translation: &str,
  glossary: &[(String, String)],
) -> Vec<String> {
  let haystack = normalize_for_matching(translation);
  let mut missing: Vec<String> = Vec::new();

  for number in extract_numbers(source) {
    let needle = normalize_for_matching(&number);
    if needle.is_empty() {
      continue;
    }
    if contains_number(&haystack, &needle) {
      continue;
    }
    if glossary_covers(&needle, glossary, &haystack) {
      continue;
    }
    if !missing.contains(&number) {
      missing.push(number);
    }
  }

  missing
}

/// Whitespace, thousands separators and full-width punctuation are irrelevant for numeric
/// matching (`90 ％` and `2,024` must match `90%` and `2024`).
fn normalize_for_matching(text: &str) -> String {
  text
    .chars()
    .filter_map(|ch| match ch {
      '％' => Some('%'),
      '，' | ',' => None,
      c if c.is_whitespace() => None,
      c => Some(c),
    })
    .collect()
}

fn contains_number(haystack: &str, needle: &str) -> bool {
  let mut offset = 0;
  while let Some(position) = haystack[offset..].find(needle) {
    let absolute = offset + position;
    let before = haystack[..absolute].chars().next_back();
    let after = haystack[absolute + needle.len()..].chars().next();
    if is_number_boundary(before) && is_number_boundary(after) {
      return true;
    }
    offset = absolute + 1;
  }
  false
}

fn is_number_boundary(ch: Option<char>) -> bool {
  !matches!(ch, Some(c) if c.is_ascii_digit() || c == '.')
}

fn glossary_covers(number: &str, glossary: &[(String, String)], haystack: &str) -> bool {
  glossary.iter().any(|(source, target)| {
    let source = normalize_for_matching(source);
    let target = normalize_for_matching(target);
    !target.is_empty() && contains_number(&source, number) && haystack.contains(&target)
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item(id: usize, zh: &str) -> TranslationItem {
    TranslationItem {
      id,
      zh: zh.to_string(),
    }
  }

  #[test]
  fn accepts_a_response_that_matches_ids_order_and_non_empty_text() {
    let response = vec![item(1, "一。"), item(2, "二。"), item(3, "三。")];

    assert_eq!(validate_block_response(&[1, 2, 3], &response), Ok(()));
  }

  #[test]
  fn rejects_a_missing_id() {
    let response = vec![item(1, "一。"), item(3, "三。")];

    assert_eq!(
      validate_block_response(&[1, 2, 3], &response),
      Err(ValidationError::MissingId(2))
    );
  }

  #[test]
  fn rejects_out_of_order_ids() {
    let response = vec![item(2, "二。"), item(1, "一。"), item(3, "三。")];

    assert_eq!(
      validate_block_response(&[1, 2, 3], &response),
      Err(ValidationError::OutOfOrder {
        expected: 1,
        found: 2,
      })
    );
  }

  #[test]
  fn rejects_an_empty_translation() {
    let response = vec![item(1, "一。"), item(2, "   "), item(3, "三。")];

    assert_eq!(
      validate_block_response(&[1, 2, 3], &response),
      Err(ValidationError::EmptyTranslation(2))
    );
  }

  #[test]
  fn rejects_unexpected_and_duplicate_ids() {
    assert_eq!(
      validate_block_response(
        &[1, 2],
        &[item(1, "一。"), item(2, "二。"), item(9, "九。")]
      ),
      Err(ValidationError::UnexpectedId(9))
    );
    // A duplicated id means some other id is missing; the missing id is the useful signal.
    assert_eq!(
      validate_block_response(
        &[1, 2, 3],
        &[item(1, "一。"), item(1, "一。"), item(3, "三。")]
      ),
      Err(ValidationError::MissingId(2))
    );
  }

  #[test]
  fn extraction_keeps_separators_percent_and_order() {
    assert_eq!(
      extract_numbers("In 2024, 2,024 people and 3.5% growth, plus 90% again"),
      vec!["2024", "2,024", "3.5%", "90%"]
    );
    assert_eq!(extract_numbers("no numbers here"), Vec::<String>::new());
  }

  #[test]
  fn fidelity_flags_a_missing_number_only() {
    let source = "In 2024 growth was 90% for the team.";
    assert!(check_numeric_fidelity(source, "2024 年团队增长了 90%。", &[]).is_empty());
    assert_eq!(
      check_numeric_fidelity(source, "2024 年团队增长了。", &[]),
      vec!["90%"]
    );
  }

  #[test]
  fn fidelity_tolerates_spacing_full_width_and_thousands_separators() {
    let source = "2,024 users and 90% uptime";
    assert!(check_numeric_fidelity(source, "2,024 位用户，90 ％ 可用性", &[]).is_empty());
    assert!(check_numeric_fidelity(source, "2024 位用户，90% 可用性", &[]).is_empty());
  }

  #[test]
  fn fidelity_does_not_accept_a_number_inside_a_longer_one() {
    let source = "about 90% of them";

    assert_eq!(
      check_numeric_fidelity(source, "大约 190% 的人", &[]),
      vec!["90%"]
    );
  }

  #[test]
  fn fidelity_accepts_a_glossary_mapping_of_the_number() {
    let glossary = vec![("90 percent".to_string(), "百分之九十".to_string())];

    assert!(check_numeric_fidelity("90 percent of people", "百分之九十的人", &glossary).is_empty());
    assert_eq!(
      check_numeric_fidelity("90 percent of people", "九成的人", &glossary),
      vec!["90"]
    );
  }

  #[test]
  fn extra_json_fields_are_rejected_by_the_contract_type() {
    let parsed = serde_json::from_str::<TranslationItem>(r#"{"id":1,"zh":"一。","note":"x"}"#);

    assert!(parsed.is_err());
  }
}
