pub mod midi;
pub mod muscriptor;

use std::path::Path;

use anyhow::Result;
use basic_pitch::{BasicPitch, MidiParams, NoteParams, TrimMode};

pub struct BasicPitchParams {
    pub onset_threshold: f64,
    pub frame_threshold: f64,
    pub minimum_note_length_ms: f64,
}

/// Transcribes a file with Basic Pitch and returns a Standard MIDI File.
///
/// # Errors
///
/// Returns an error when the audio cannot be decoded or the model fails.
pub fn basic_pitch(wav: &Path, params: &BasicPitchParams) -> Result<Vec<u8>> {
    let audio = audio::decode(wav)?.mono(22050)?;
    let transcriber = BasicPitch::new()?
        .with_trim_mode(TrimMode::V0_4_0)
        .with_threads(std::thread::available_parallelism()?);
    let transcription = transcriber.transcribe_with(
        audio.as_slice().unwrap_or_default(),
        &NoteParams {
            onset_threshold: params.onset_threshold,
            frame_threshold: params.frame_threshold,
            minimum_note_length_ms: params.minimum_note_length_ms,
            ..NoteParams::default()
        },
    )?;
    Ok(transcription.to_midi(&MidiParams::default())?)
}
