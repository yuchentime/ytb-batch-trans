use crate::models::download::{DownloadOverrides, PlaylistMode};
use crate::runners::override_resolver::resolve_with_patch;
use crate::state::config_models::Config;
use crate::stronghold::stronghold_state::AuthSecrets;

use super::{build_auth_args, build_network_args};

/// Audio-only format selector: cheapest audio stream, falling back to the best combined
/// stream when a video has no audio-only format (design §2).
pub const AUDIO_FORMAT_SELECTOR: &str = "ba/best";

/// Builds the complete yt-dlp argv for the audio-only fetch of a single video (AC-02).
///
/// Deliberately audio-only: no merging/remux/re-encode, no post-processing, no subtitles,
/// no SponsorBlock, no input filters. Network and auth settings are resolved from `config`
/// plus `overrides` exactly like the download runner does (M1/M2); `secrets` must come from
/// the stronghold vault.
///
/// The transcribe pipeline wires this in the next loop; until then the legacy download
/// runner keeps its own composition, hence the temporary `dead_code` allowance on the
/// module declaration in `ytdlp_args.rs`.
pub fn build_audio_download_args(
  config: &Config,
  overrides: Option<&DownloadOverrides>,
  output_path: &str,
  secrets: &AuthSecrets,
) -> Vec<String> {
  let mut args = vec![
    "-f".to_string(),
    AUDIO_FORMAT_SELECTOR.to_string(),
    "-o".to_string(),
    output_path.to_string(),
    playlist_flag(config, overrides).to_string(),
  ];

  let network = resolve_with_patch(
    &config.network,
    overrides.and_then(|value| value.network.as_ref()),
  );
  args.extend(build_network_args(&network));

  let auth_overrides = overrides.and_then(|value| value.auth.as_ref());
  let auth = resolve_with_patch(&config.auth, auth_overrides);
  let secrets = resolve_with_patch(secrets, auth_overrides);
  args.extend(build_auth_args(&auth, &secrets));

  args
}

/// Playlist handling mirrors `YtdlpRunner::with_input_args`: an explicit per-group
/// override wins, otherwise the global mixed-link preference decides.
fn playlist_flag(config: &Config, overrides: Option<&DownloadOverrides>) -> &'static str {
  let playlist_mode = overrides
    .and_then(|value| value.input_filters.as_ref())
    .and_then(|value| value.playlist_mode);

  match playlist_mode {
    Some(PlaylistMode::SingleVideo) => "--no-playlist",
    Some(PlaylistMode::Playlist) => "--yes-playlist",
    None => {
      let input = resolve_with_patch(
        &config.input,
        overrides.and_then(|value| value.input.as_ref()),
      );
      if input.prefer_video_in_mixed_links {
        "--no-playlist"
      } else {
        "--yes-playlist"
      }
    }
  }
}
