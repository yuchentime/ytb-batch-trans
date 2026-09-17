use crate::SharedConfig;
use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::{collections::HashMap, sync::RwLock};
use tauri::{AppHandle, Manager};

pub const FALLBACK_LOCALE: &str = "en";
pub const SIMPLIFIED_CHINESE_LOCALE: &str = "zh-CN";

static LOCALES_DIR: Dir = include_dir!("./locales");

#[derive(Debug)]
pub struct I18nManager {
  locales: HashMap<String, Value>,
  current_locale: RwLock<String>,
}

impl I18nManager {
  pub fn new(app: &AppHandle) -> Self {
    let locales = load_locales_from_embedded();

    let cfg_handle = app.state::<SharedConfig>();
    let cfg = cfg_handle.load();
    let initial = resolve_locale_from_sources(&locales, Some(cfg.appearance.language.as_str()));

    Self {
      locales,
      current_locale: RwLock::new(initial),
    }
  }

  pub fn set_locale(&self, locale: &str) -> bool {
    let norm = normalize_locale_code(locale);

    let resolved = match resolve_known_locale(&self.locales, &norm) {
      Some(resolved) => resolved,
      None => return false,
    };

    let mut guard = self
      .current_locale
      .write()
      .expect("i18n current_locale lock poisoned");

    if *guard == resolved {
      return false;
    }

    *guard = resolved;
    true
  }

  pub fn unset_locale(&self) -> bool {
    let locale = resolve_locale_from_sources(&self.locales, None);
    self.set_locale(&locale)
  }

  pub fn t(&self, key: &str) -> String {
    let locale = self
      .current_locale
      .read()
      .expect("i18n locale lock poisoned")
      .clone();

    self.t_in(&locale, key).unwrap_or_else(|| key.to_string())
  }

  pub fn t_in(&self, locale: &str, key: &str) -> Option<String> {
    if let Some(s) = lookup_str(self.locale_root(locale), key) {
      return Some(s.to_string());
    }

    if locale != FALLBACK_LOCALE {
      if let Some(s) = lookup_str(self.locale_root(FALLBACK_LOCALE), key) {
        return Some(s.to_string());
      }
    }

    None
  }

  pub fn t_with(&self, key: &str, params: Option<&HashMap<String, String>>) -> String {
    let raw = self.t(key);

    let mut s = if raw.contains('|') {
      if let Some(count) = params
        .and_then(|p| p.get("n"))
        .and_then(|v| v.trim().parse::<i64>().ok())
      {
        select_plural_variant(&raw, count)
      } else {
        raw
      }
    } else {
      raw
    };

    if let Some(params) = params {
      for (k, v) in params {
        let needle = format!("{{{k}}}");
        s = s.replace(&needle, v);
      }
    }

    s
  }

  fn locale_root(&self, locale: &str) -> &Value {
    self.locales.get(locale).unwrap_or_else(|| {
      self
        .locales
        .get(FALLBACK_LOCALE)
        .expect("fallback must exist")
    })
  }
}

fn load_locales_from_embedded() -> HashMap<String, Value> {
  let mut out = HashMap::<String, Value>::new();

  for file in LOCALES_DIR.files() {
    let path = file.path();
    if path.extension().and_then(|s| s.to_str()) != Some("json") {
      continue;
    }

    let locale_code = match path.file_stem().and_then(|s| s.to_str()) {
      Some(stem) if !stem.is_empty() => stem.to_string(),
      _ => continue,
    };

    let json: Value = serde_json::from_slice(file.contents()).unwrap_or_else(|err| {
      panic!(
        "Failed to parse locale JSON for '{locale_code}' ({}): {err}",
        path.display()
      )
    });

    out.insert(locale_code, json);
  }

  if !out.contains_key(FALLBACK_LOCALE) {
    panic!("Missing fallback locale '{FALLBACK_LOCALE}' in embedded locales folder.");
  }

  out
}

fn lookup_str<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
  let mut cur = root;
  for part in key.split('.') {
    cur = cur.get(part)?;
  }
  cur.as_str()
}

pub fn normalize_locale_code(input: &str) -> String {
  let s = input.trim().replace('_', "-");
  if s.is_empty() {
    return FALLBACK_LOCALE.to_string();
  }

  let mut parts: Vec<String> = s.split('-').map(|p| p.to_string()).collect();
  parts[0] = parts[0].to_lowercase();

  if parts.len() >= 2 && parts[1].len() == 2 {
    parts[1] = parts[1].to_uppercase();
  }

  parts.join("-")
}

fn resolve_locale_from_sources(
  locales: &HashMap<String, Value>,
  config_locale: Option<&str>,
) -> String {
  if let Some(raw) = config_locale {
    let raw = raw.trim();
    if !raw.is_empty() {
      let norm = normalize_locale_code(raw);

      if let Some(locale) = resolve_known_locale(locales, &norm) {
        return locale;
      }
    }
  }

  if let Some(sys) = sys_locale::get_locale() {
    let norm = normalize_locale_code(&sys);

    if let Some(locale) = resolve_known_locale(locales, &norm) {
      return locale;
    }
  }

  FALLBACK_LOCALE.to_string()
}

/// Resolve a normalized locale code against the embedded packs: exact match first,
/// then the language-only code, then the Simplified Chinese pack for any Chinese
/// variant (the app no longer ships a Traditional Chinese UI).
fn resolve_known_locale(locales: &HashMap<String, Value>, normalized: &str) -> Option<String> {
  if locales.contains_key(normalized) {
    return Some(normalized.to_string());
  }

  if let Some(primary) = normalized.split('-').next() {
    if locales.contains_key(primary) {
      return Some(primary.to_string());
    }
  }

  if (normalized == "zh" || normalized.starts_with("zh-"))
    && locales.contains_key(SIMPLIFIED_CHINESE_LOCALE)
  {
    return Some(SIMPLIFIED_CHINESE_LOCALE.to_string());
  }

  None
}

fn select_plural_variant(raw: &str, count: i64) -> String {
  if !raw.contains('|') {
    return raw.to_string();
  }

  let parts: Vec<&str> = raw
    .split('|')
    .map(|p| p.trim())
    .filter(|p| !p.is_empty())
    .collect();

  match parts.len() {
    0 => raw.to_string(),
    1 => parts[0].to_string(),

    2 => {
      if count == 1 {
        parts[0].to_string()
      } else {
        parts[1].to_string()
      }
    }

    3 => {
      if count == 0 {
        parts[0].to_string()
      } else if count == 1 {
        parts[1].to_string()
      } else {
        parts[2].to_string()
      }
    }

    _ => {
      if count == 0 {
        parts[0].to_string()
      } else if count == 1 {
        parts[1].to_string()
      } else {
        parts[parts.len() - 1].to_string()
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  fn locale_packs() -> HashMap<String, Value> {
    HashMap::from([
      ("en".to_string(), json!({})),
      (SIMPLIFIED_CHINESE_LOCALE.to_string(), json!({})),
    ])
  }

  #[test]
  fn chinese_variants_resolve_to_the_simplified_pack() {
    let locales = locale_packs();

    for code in ["zh", "zh-Hans", "zh-CN", "zh-TW", "zh-Hant", "zh-HK"] {
      assert_eq!(
        resolve_known_locale(&locales, &normalize_locale_code(code)),
        Some(SIMPLIFIED_CHINESE_LOCALE.to_string()),
        "expected {code} to resolve to the Simplified Chinese pack"
      );
    }
  }

  #[test]
  fn exact_locale_match_wins_over_the_language_code() {
    let mut locales = locale_packs();
    locales.insert("zh-TW".to_string(), json!({}));

    assert_eq!(
      resolve_known_locale(&locales, "zh-TW"),
      Some("zh-TW".to_string())
    );
  }

  #[test]
  fn unknown_locale_is_not_resolved() {
    assert_eq!(resolve_known_locale(&locale_packs(), "fr"), None);
  }
}
