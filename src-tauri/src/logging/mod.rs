// Event codes are a stable contract consumed by the transcribe pipeline (next loops);
// the allowance disappears when the pipeline emits them.
#[allow(dead_code)]
pub mod events;
pub mod file_log;
pub mod log_state;
pub mod log_store;

pub use log_state::*;
pub use log_store::*;
