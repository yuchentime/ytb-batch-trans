use crate::state::config_models::AuthSettings;
use crate::stronghold::stronghold_state::AuthSecrets;

/// Cookie and credential flags (M1: restricted videos must keep working).
///
/// Pure function shared by [`super::audio_args::build_audio_download_args`] and the
/// runners. The caller is responsible for reading `secrets` from the stronghold vault and
/// for applying overrides — secret values never leave this argv construction.
pub fn build_auth_args(auth: &AuthSettings, secrets: &AuthSecrets) -> Vec<String> {
  let mut args = Vec::new();

  if auth.cookie_browser != "none" {
    args.push("--cookies-from-browser".to_string());
    args.push(auth.cookie_browser.clone());
  }
  if let Some(cookie_file) = auth.cookie_file.as_ref() {
    args.push("--cookies".to_string());
    args.push(cookie_file.clone());
  }

  if let Some(username) = secrets.username.as_ref() {
    args.push("--username".to_string());
    args.push(username.clone());
  }
  if let Some(password) = secrets.password.as_ref() {
    args.push("--password".to_string());
    args.push(password.clone());
  }
  if let Some(video_password) = secrets.video_password.as_ref() {
    args.push("--video-password".to_string());
    args.push(video_password.clone());
  }
  if let Some(token) = secrets.bearer_token.as_ref() {
    args.push("--add-header".to_string());
    args.push(format!("Authorization:Bearer {token}"));
  }
  for header in &secrets.headers {
    args.push("--add-header".to_string());
    args.push(header.clone());
  }

  args
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn auth_args_include_cookies_and_secrets_in_stable_order() {
    let auth = AuthSettings {
      cookie_file: Some("/tmp/cookies.txt".into()),
      cookie_browser: "firefox".into(),
    };
    let secrets = AuthSecrets {
      username: Some("user".into()),
      password: Some("pass".into()),
      video_password: Some("video".into()),
      bearer_token: Some("token".into()),
      headers: vec!["X-Test: 1".into()],
    };

    let args = build_auth_args(&auth, &secrets);

    assert_eq!(
      args,
      vec![
        "--cookies-from-browser",
        "firefox",
        "--cookies",
        "/tmp/cookies.txt",
        "--username",
        "user",
        "--password",
        "pass",
        "--video-password",
        "video",
        "--add-header",
        "Authorization:Bearer token",
        "--add-header",
        "X-Test: 1",
      ]
    );
  }

  #[test]
  fn auth_args_are_empty_without_configuration() {
    let args = build_auth_args(&AuthSettings::default(), &AuthSecrets::default());

    assert!(args.is_empty());
  }
}
