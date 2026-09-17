// `audio_args` is wired into the transcribe pipeline in the next loop; until then its public
// surface would be reported as dead code. Drop the allowance with the wiring (Phase A, L003).
#[allow(dead_code)]
mod audio_args;
mod auth_args;
mod format_args;
mod input_filter_args;
mod location_args;
mod network_args;
mod output_args;

#[allow(unused_imports)]
pub use audio_args::{build_audio_download_args, AUDIO_FORMAT_SELECTOR};
pub use auth_args::build_auth_args;
pub use format_args::build_format_args;
pub use input_filter_args::build_input_filter_args;
pub use location_args::build_location_args;
pub use network_args::build_network_args;
pub use output_args::build_output_args;

#[cfg(test)]
mod tests;
