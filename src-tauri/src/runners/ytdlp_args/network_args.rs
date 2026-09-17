use crate::state::config_models::NetworkSettings;

/// Network-related yt-dlp flags (M2: proxy / impersonate / extractor-args keep working).
///
/// Pure function shared by [`super::audio_args::build_audio_download_args`] and the
/// download/info runners, so all call sites produce the same argv.
pub fn build_network_args(network: &NetworkSettings) -> Vec<String> {
  let mut args = Vec::new();

  let proxy_enabled = network.enable_proxy.is_some_and(|enabled| enabled);
  if proxy_enabled {
    if let Some(proxy) = network.proxy.as_ref() {
      args.push("--proxy".to_string());
      args.push(proxy.clone());
    }
  }

  match network.impersonate.as_str() {
    "none" => {}
    "any" => {
      args.push("--impersonate".to_string());
      args.push(String::new());
    }
    other => {
      args.push("--impersonate".to_string());
      args.push(other.to_string());
    }
  }

  if let Some(extractor_args) = normalize_extractor_args(&network.extractor_args) {
    args.push("--extractor-args".into());
    args.push(extractor_args);
  }

  args
}

/// Collapses the multi-line `--extractor-args` text field into a single argument.
fn normalize_extractor_args(value: &str) -> Option<String> {
  let normalized = value
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
  if normalized.is_empty() {
    None
  } else {
    Some(normalized)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extractor_args_empty_returns_none() {
    assert_eq!(normalize_extractor_args(""), None);
    assert_eq!(normalize_extractor_args("   "), None);
  }

  #[test]
  fn extractor_args_trim_whitespace() {
    assert_eq!(
      normalize_extractor_args(" youtube:player_js_variant=main "),
      Some("youtube:player_js_variant=main".into())
    );
  }

  #[test]
  fn extractor_args_join_non_empty_lines_with_spaces() {
    assert_eq!(
      normalize_extractor_args(
        " youtube:player_js_variant=main \n\n youtube:skip=hls,dash \r\n generic:impersonate"
      ),
      Some("youtube:player_js_variant=main youtube:skip=hls,dash generic:impersonate".into())
    );
  }

  #[test]
  fn network_args_include_proxy_impersonate_and_extractor_args() {
    let network = NetworkSettings {
      enable_proxy: Some(true),
      proxy: Some("http://127.0.0.1:8080".into()),
      impersonate: "chrome".into(),
      extractor_args: "youtube:player_js_variant=main".into(),
    };

    let args = build_network_args(&network);

    assert_eq!(
      args,
      vec![
        "--proxy",
        "http://127.0.0.1:8080",
        "--impersonate",
        "chrome",
        "--extractor-args",
        "youtube:player_js_variant=main",
      ]
    );
  }

  #[test]
  fn network_args_are_empty_without_configuration() {
    let network = NetworkSettings {
      enable_proxy: Some(false),
      proxy: Some("http://127.0.0.1:8080".into()),
      ..NetworkSettings::default()
    };

    assert!(build_network_args(&network).is_empty());
  }
}
