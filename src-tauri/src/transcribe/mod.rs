//! Pure transcription helpers for the transcribe pipeline (design §3/§4).
//!
//! Consumed by `scheduling::transcribe_pipeline` (L008); remove the `dead_code`
//! allowances with the wiring.

#[allow(dead_code)]
pub mod chunking;
#[allow(dead_code)]
pub mod merge;
#[allow(dead_code)]
pub mod transcript;
