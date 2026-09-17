//! Translation helpers for the transcribe pipeline (design §5).
//!
//! Consumed by `scheduling::transcribe_pipeline` (L008); remove the `dead_code`
//! allowances with the wiring.

#[allow(dead_code)]
pub mod blocks;
#[allow(dead_code)]
pub mod deepseek_client;
#[allow(dead_code)]
pub mod validate;
