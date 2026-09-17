//! Pure transcription helpers for the transcribe pipeline (design §3/§4).
//!
//! Consumed by `scheduling::transcribe_pipeline`.

pub mod artifacts;
pub mod chunking;
pub mod merge;
pub mod transcript;
