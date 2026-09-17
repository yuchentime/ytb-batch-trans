//! `transcription_probe`: environment gate for the transcribe UI (AC-01/AC-27, cuts C2/C3).
//!
//! Reports whisper/ffmpeg/ffprobe availability, CUDA presence, model cache state, whether
//! the DeepSeek key is configured (no connectivity check, cut C3) and the file-log path.
//! Everything is read-only: nothing is written and no model is downloaded (cut C2).

use crate::logging::events;
use crate::logging::file_log;
use crate::paths::PathsManager;
use crate::runners::ytdlp_process::configure_command;
use crate::stronghold::stronghold_state::StrongholdState;
use crate::SharedConfig;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionProbe {
  pub whisper_path: Option<String>,
  pub whisper_version: Option<String>,
  pub whisper_found: bool,
  pub cuda_available: bool,
  pub cuda_device: Option<String>,
  pub model: String,
  pub model_cached: bool,
  pub model_path: Option<String>,
  pub model_size_bytes: Option<u64>,
  pub ffmpeg_path: Option<String>,
  pub ffprobe_path: Option<String>,
  pub api_key_configured: bool,
  pub log_dir: String,
  pub log_file: String,
  pub log_size_bytes: u64,
}

#[tauri::command]
pub async fn transcription_probe(app: AppHandle) -> Result<TranscriptionProbe, String> {
  let cfg = app.state::<SharedConfig>().load();
  let bin_dir = app.state::<PathsManager>().bin_dir().clone();

  let whisper = resolve_program(
    cfg.transcription.whisper_path.as_deref(),
    &bin_dir,
    "whisper",
  );
  let whisper_found = whisper.is_some();
  let whisper_version = match whisper.as_deref() {
    Some(program) => probe_whisper_version(program).await,
    None => None,
  };

  let cuda_device = run_probe(
    Path::new("nvidia-smi"),
    &["--query-gpu=name", "--format=csv,noheader"],
  )
  .await
  .map(|output| output.lines().next().unwrap_or_default().trim().to_string())
  .filter(|name| !name.is_empty());

  let model = model_arg(cfg.transcription.model).to_string();
  let model_path = model_cache_path(&model);
  let model_size_bytes = model_path
    .as_deref()
    .and_then(|path| std::fs::metadata(path).ok())
    .map(|metadata| metadata.len());
  let model_cached = model_size_bytes.is_some();

  let app_dir = app.state::<PathsManager>().app_dir().clone();
  let api_key_configured = app
    .state::<StrongholdState>()
    .load_ai_api_key()
    .map(|key| key.is_some())
    .unwrap_or(false);

  let whisper_field = whisper
    .as_deref()
    .map(|path| path.display().to_string())
    .unwrap_or_else(|| "missing".to_string());
  if whisper_found {
    tracing::info!(
      event = events::PROBE_OK,
      whisper = %whisper_field,
      cuda = cuda_device.is_some(),
      model = %model,
      cached = model_cached,
      has_api_key = api_key_configured,
    );
  } else {
    tracing::error!(
      event = events::PROBE_MISSING,
      whisper = %whisper_field,
      cuda = cuda_device.is_some(),
      model = %model,
    );
  }

  Ok(TranscriptionProbe {
    whisper_path: whisper.map(|path| path.display().to_string()),
    whisper_version,
    whisper_found,
    cuda_available: cuda_device.is_some(),
    cuda_device,
    model,
    model_cached,
    model_path: model_path.map(|path| path.display().to_string()),
    model_size_bytes,
    ffmpeg_path: resolve_program(None, &bin_dir, "ffmpeg").map(|path| path.display().to_string()),
    ffprobe_path: resolve_program(None, &bin_dir, "ffprobe").map(|path| path.display().to_string()),
    api_key_configured,
    log_dir: file_log::log_dir(&app_dir).display().to_string(),
    log_file: file_log::log_file_path(&app_dir).display().to_string(),
    log_size_bytes: file_log::log_size_bytes(&app_dir),
  })
}

/// Cheap gate for `transcribe_start` (AC-01): does not spawn whisper, only resolves it.
pub(crate) fn whisper_found(app: &AppHandle) -> bool {
  let cfg = app.state::<SharedConfig>().load();
  let bin_dir = app.state::<PathsManager>().bin_dir().clone();
  resolve_program(
    cfg.transcription.whisper_path.as_deref(),
    &bin_dir,
    "whisper",
  )
  .is_some()
}

/// Resolves an executable from an explicit override, the toolchain `bin_dir`, or `PATH`.
/// On Windows the usual script extensions are tried as well (`npm`/`pip` shims).
pub(crate) fn resolve_program(
  configured: Option<&str>,
  bin_dir: &Path,
  name: &str,
) -> Option<PathBuf> {
  if let Some(value) = configured.map(str::trim).filter(|value| !value.is_empty()) {
    let path = PathBuf::from(value);
    return path.is_file().then_some(path);
  }

  let path_variable = std::env::var_os("PATH").unwrap_or_default();
  let directories =
    std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&path_variable));

  for directory in directories {
    for candidate in executable_names(name) {
      let candidate = directory.join(candidate);
      if candidate.is_file() {
        return Some(candidate);
      }
    }
  }
  None
}

fn executable_names(name: &str) -> Vec<String> {
  #[cfg(windows)]
  {
    vec![
      format!("{name}.exe"),
      format!("{name}.cmd"),
      format!("{name}.bat"),
      name.to_string(),
    ]
  }
  #[cfg(not(windows))]
  {
    vec![name.to_string()]
  }
}

fn model_arg(model: crate::state::config_models::TranscriptionModel) -> &'static str {
  use crate::state::config_models::TranscriptionModel;
  match model {
    TranscriptionModel::Small => "small",
    TranscriptionModel::Medium => "medium",
    TranscriptionModel::LargeV3 => "large-v3",
  }
}

fn model_cache_path(model: &str) -> Option<PathBuf> {
  let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
  Some(
    PathBuf::from(home)
      .join(".cache")
      .join("whisper")
      .join(format!("{model}.pt")),
  )
}

async fn probe_whisper_version(program: &Path) -> Option<String> {
  // `whisper --help` proves the executable actually runs; the version comes from the pip
  // metadata because the CLI itself has no `--version`.
  let help = run_probe(program, &["--help"]).await?;
  if !help.to_lowercase().contains("usage") {
    return None;
  }

  let version = run_probe(
    Path::new("python"),
    &["-m", "pip", "show", "openai-whisper"],
  )
  .await
  .and_then(|output| {
    output.lines().find_map(|line| {
      line
        .strip_prefix("Version:")
        .map(|value| value.trim().to_string())
    })
  });

  version
}

/// Runs a short probe process off the async runtime and returns its stdout (or stderr when
/// stdout is empty) on success.
async fn run_probe(program: &Path, args: &[&str]) -> Option<String> {
  let program = program.to_path_buf();
  let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();

  let output = tauri::async_runtime::spawn_blocking(move || {
    let mut command = Command::new(&program);
    command.args(&args);
    let _ = configure_command(&mut command);
    command.output()
  })
  .await
  .ok()?
  .ok()?;

  if !output.status.success() {
    return None;
  }
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if !stdout.is_empty() {
    return Some(stdout);
  }
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
  (!stderr.is_empty()).then_some(stderr)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn executable_names_cover_windows_shims() {
    let names = executable_names("whisper");
    assert!(names.contains(&"whisper".to_string()));
    #[cfg(windows)]
    {
      assert!(names.contains(&"whisper.exe".to_string()));
      assert!(names.contains(&"whisper.cmd".to_string()));
    }
  }

  #[test]
  fn explicit_program_must_exist() {
    let bin_dir = std::env::temp_dir();

    assert!(resolve_program(Some("/definitely/not/here/whisper"), &bin_dir, "whisper").is_none());
  }

  #[test]
  fn model_cache_path_uses_the_whisper_cache_directory() {
    let path = model_cache_path("large-v3").expect("home directory");
    assert!(path.ends_with(Path::new(".cache").join("whisper").join("large-v3.pt")));
  }

  #[test]
  fn model_names_match_the_whisper_release() {
    use crate::state::config_models::TranscriptionModel;
    assert_eq!(model_arg(TranscriptionModel::Small), "small");
    assert_eq!(model_arg(TranscriptionModel::Medium), "medium");
    assert_eq!(model_arg(TranscriptionModel::LargeV3), "large-v3");
  }
}
