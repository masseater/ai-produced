use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hf_hub::HFClientSync;
use tract_onnx::prelude::*;

use crate::midi::{DRUM_PROGRAM, Note};

const REPO_OWNER: &str = "unifolia";
const REPO_NAME: &str = "muscriptor-small-onnx";
const REVISION: &str = "15dd43dd431b7b12c0fb2f771bb57dea0cc2d7fa";
pub const SAMPLE_RATE: u32 = 16000;
const SEGMENT: usize = 80000;
const SEGMENT_SECONDS: f64 = 5.0;
const MAX_GEN: usize = 2000;
const INITIAL: i64 = 1393;
const EOS: usize = 1;
const FRAME_RATE: f64 = 100.0;
const MINIMUM_NOTE_SECONDS: f64 = 0.01;
const SHIFT: usize = 3;
const PITCH: usize = 1004;
const VELOCITY: usize = 1132;
const TIE: usize = 1134;
const PROGRAM: usize = 1135;
const DRUM: usize = 1265;
const VOCAB: usize = 1393;
const DIM: usize = 768;
const LAYERS: usize = 14;
const HEADS: usize = 12;
const HEAD_DIM: usize = 64;

const GROUPS: [(&str, u16); 34] = [
    ("acoustic_piano", 0),
    ("electric_piano", 2),
    ("chromatic_percussion", 8),
    ("organ", 16),
    ("acoustic_guitar", 24),
    ("clean_electric_guitar", 26),
    ("distorted_electric_guitar", 29),
    ("acoustic_bass", 32),
    ("electric_bass", 33),
    ("violin", 40),
    ("viola", 41),
    ("cello", 42),
    ("contrabass", 43),
    ("orchestral_harp", 46),
    ("timpani", 47),
    ("string_ensemble", 48),
    ("synth_strings", 50),
    ("voice", 52),
    ("orchestra_hit", 55),
    ("trumpet", 56),
    ("trombone", 57),
    ("tuba", 58),
    ("french_horn", 60),
    ("brass_section", 61),
    ("soprano_and_alto_sax", 64),
    ("tenor_sax", 66),
    ("baritone_sax", 67),
    ("oboe", 68),
    ("english_horn", 69),
    ("bassoon", 70),
    ("clarinet", 71),
    ("flutes", 72),
    ("synth_lead", 80),
    ("synth_pad", 88),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Other,
    Shift(u32),
    Pitch(u8),
    Velocity(u8),
    Tie,
    Program(u16),
    Drum(u8),
}

fn token(id: usize) -> Token {
    match id {
        SHIFT..PITCH => Token::Shift((id - SHIFT) as u32),
        PITCH..VELOCITY => Token::Pitch((id - PITCH) as u8),
        VELOCITY..TIE => Token::Velocity((id - VELOCITY) as u8),
        TIE => Token::Tie,
        PROGRAM..DRUM => Token::Program((id - PROGRAM) as u16),
        DRUM..VOCAB => Token::Drum((id - DRUM) as u8),
        _ => Token::Other,
    }
}

#[must_use]
pub fn instrument_name(program: u16) -> String {
    if program == DRUM_PROGRAM {
        return "drums".to_string();
    }
    GROUPS.iter().find(|(_, p)| *p == program).map_or_else(
        || format!("program {program}"),
        |(name, _)| name.replace('_', " "),
    )
}

fn tie_prologue(open: &[(u16, u8)]) -> Vec<usize> {
    let mut tokens = Vec::new();
    let mut current = None;
    for &(program, pitch) in open {
        if current != Some(program) {
            tokens.push(PROGRAM + program as usize);
            current = Some(program);
        }
        tokens.push(PITCH + pitch as usize);
    }
    tokens.push(TIE);
    tokens
}

enum Action {
    Start(u16, u8, f64),
    End(u16, u8, f64),
    Hit(u8, f64),
}

#[derive(Default)]
struct Tracker {
    open: Vec<((u16, u8), f64)>,
    seek: f64,
    next_seek: Option<f64>,
    start_tick: i64,
    tick: i64,
    program: Option<u16>,
    velocity: Option<u8>,
    in_prologue: bool,
    skip_rest: bool,
    tie_set: BTreeSet<(u16, u8)>,
    started: bool,
}

impl Tracker {
    fn end_all(&mut self, time: f64) -> Vec<Action> {
        self.open
            .drain(..)
            .map(|((program, pitch), _)| Action::End(program, pitch, time))
            .collect()
    }

    fn boundary(&mut self, seek: f64, next_seek: Option<f64>) -> Vec<Action> {
        let actions = if self.started && self.in_prologue {
            self.end_all(self.seek)
        } else {
            Vec::new()
        };
        self.seek = seek;
        self.next_seek = next_seek;
        self.start_tick = (seek * FRAME_RATE).round_ties_even() as i64;
        self.tick = self.start_tick;
        self.program = None;
        self.velocity = None;
        self.in_prologue = true;
        self.skip_rest = false;
        self.tie_set.clear();
        self.started = true;
        actions
    }

    fn feed(&mut self, id: usize) -> Vec<Action> {
        let event = token(id);
        if self.in_prologue {
            match event {
                Token::Tie => {
                    self.in_prologue = false;
                    self.velocity = None;
                    let seek = self.seek;
                    let (kept, ended): (Vec<_>, Vec<_>) = self
                        .open
                        .drain(..)
                        .partition(|(key, _)| self.tie_set.contains(key));
                    self.open = kept;
                    return ended
                        .into_iter()
                        .map(|((program, pitch), _)| Action::End(program, pitch, seek))
                        .collect();
                }
                Token::Shift(_) => {
                    self.in_prologue = false;
                    self.skip_rest = true;
                    return self.end_all(self.seek);
                }
                Token::Program(program) => self.program = Some(program),
                Token::Pitch(pitch) => {
                    if let Some(program) = self.program {
                        self.tie_set.insert((program, pitch));
                    }
                }
                _ => {}
            }
            return Vec::new();
        }
        if self.skip_rest {
            return Vec::new();
        }
        let time = self.tick as f64 / FRAME_RATE;
        let inside = self.next_seek.is_none_or(|next| time < next);
        match event {
            Token::Shift(value) if value > 0 => self.tick = self.start_tick + i64::from(value),
            Token::Program(program) => self.program = Some(program),
            Token::Velocity(velocity) => self.velocity = Some(velocity),
            Token::Drum(pitch) if inside => return vec![Action::Hit(pitch, time)],
            Token::Pitch(pitch) => {
                let (Some(program), Some(velocity)) = (self.program, self.velocity) else {
                    return Vec::new();
                };
                if !inside {
                    return Vec::new();
                }
                let key = (program, pitch);
                let mut actions = Vec::new();
                if let Some(at) = self.open.iter().position(|(k, _)| *k == key) {
                    self.open.remove(at);
                    actions.push(Action::End(program, pitch, time));
                }
                if velocity > 0 {
                    self.open.push((key, time));
                    actions.push(Action::Start(program, pitch, time));
                }
                return actions;
            }
            _ => {}
        }
        Vec::new()
    }

    fn finish(&mut self) -> Vec<Action> {
        if self.started && self.in_prologue {
            return self.end_all(self.seek);
        }
        self.open
            .drain(..)
            .map(|((program, pitch), onset)| {
                Action::End(program, pitch, onset + MINIMUM_NOTE_SECONDS)
            })
            .collect()
    }

    fn open_keys(&self) -> Vec<(u16, u8)> {
        let mut keys: Vec<(u16, u8)> = self.open.iter().map(|(key, _)| *key).collect();
        keys.sort_unstable();
        keys
    }
}

#[derive(Default)]
struct Notes {
    open: HashMap<(u16, u8), f64>,
    done: Vec<Note>,
}

impl Notes {
    fn apply(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::Start(program, pitch, time) => {
                    self.open.insert((program, pitch), time);
                }
                Action::End(program, pitch, time) => {
                    if let Some(onset) = self.open.remove(&(program, pitch)) {
                        self.done.push(Note {
                            program,
                            pitch,
                            onset,
                            offset: time,
                        });
                    }
                }
                Action::Hit(pitch, time) => self.done.push(Note {
                    program: DRUM_PROGRAM,
                    pitch,
                    onset: time,
                    offset: time + MINIMUM_NOTE_SECONDS,
                }),
            }
        }
    }
}

fn validated(mut notes: Vec<Note>) -> Vec<Note> {
    for note in &mut notes {
        if note.onset > note.offset {
            note.offset = note.offset.max(note.onset + MINIMUM_NOTE_SECONDS);
        } else if !note.is_drum() && note.offset - note.onset < 0.01 {
            note.offset = note.onset + MINIMUM_NOTE_SECONDS;
        }
    }
    notes
}

fn trimmed(notes: Vec<Note>) -> Vec<Note> {
    if notes.len() <= 1 {
        return notes;
    }
    let mut channels: HashMap<(u16, u8), Vec<Note>> = HashMap::new();
    for note in notes {
        channels
            .entry((note.program, note.pitch))
            .or_default()
            .push(note);
    }
    let mut out = Vec::new();
    for (_, mut channel) in channels {
        channel.sort_by(|a, b| a.onset.total_cmp(&b.onset));
        for i in 1..channel.len() {
            if channel[i - 1].offset > channel[i].onset {
                channel[i - 1].offset = channel[i].onset;
            }
        }
        out.extend(channel.into_iter().filter(|n| n.onset < n.offset));
    }
    out.sort_by(|a, b| {
        a.onset
            .total_cmp(&b.onset)
            .then(a.is_drum().cmp(&b.is_drum()))
            .then(a.program.cmp(&b.program))
            .then(a.pitch.cmp(&b.pitch))
            .then(a.offset.total_cmp(&b.offset))
    });
    out
}

type Plan = Arc<TypedRunnableModel>;

pub struct Muscriptor {
    conditioner: Plan,
    decoder: Plan,
}

fn download(name: &str) -> Result<PathBuf> {
    let client = HFClientSync::new()?;
    Ok(client
        .model(REPO_OWNER, REPO_NAME)
        .download_file()
        .filename(name)
        .revision(REVISION)
        .send()?)
}

fn plan(path: &Path, audio_input: bool) -> Result<Plan> {
    let mut model = tract_onnx::onnx()
        .model_for_path(path)
        .with_context(|| path.display().to_string())?;
    if audio_input {
        model = model.with_input_fact(0, f32::fact([1, SEGMENT]).into())?;
    }
    model.into_optimized()?.into_runnable()
}

impl Muscriptor {
    pub fn load() -> Result<Self> {
        Ok(Self {
            conditioner: plan(&download("muscriptor-small-cond.onnx")?, true)?,
            decoder: plan(&download("muscriptor-small-lm.onnx")?, false)?,
        })
    }

    fn generate(&self, chunk: Vec<f32>, prompt: &[usize]) -> Result<Vec<usize>> {
        let audio: Tensor = tract_ndarray::Array2::from_shape_vec((1, SEGMENT), chunk)?.into();
        let unconditional: Tensor = tract_ndarray::arr2(&[[1i64]]).into();
        let prefix = self
            .conditioner
            .run(tvec!(
                audio.into(),
                unconditional.clone().into(),
                unconditional.into()
            ))?
            .remove(0);
        let empty_prefix: TValue = tract_ndarray::Array3::<f32>::zeros((1, 0, DIM)).into_tvalue();
        let mut past_k: TValue =
            tract_ndarray::Array5::<f32>::zeros((LAYERS, 1, 0, HEADS, HEAD_DIM)).into_tvalue();
        let mut past_v = past_k.clone();
        let mut input: Vec<i64> = std::iter::once(INITIAL)
            .chain(prompt.iter().map(|t| *t as i64))
            .collect();
        let mut tokens = prompt.to_vec();
        let mut first = true;
        for _ in prompt.len()..MAX_GEN {
            let ids: Tensor =
                tract_ndarray::Array2::from_shape_vec((1, input.len()), input.clone())?.into();
            let mut out = self.decoder.run(tvec!(
                ids.into(),
                if first {
                    prefix.clone()
                } else {
                    empty_prefix.clone()
                },
                past_k,
                past_v
            ))?;
            first = false;
            past_v = out.remove(2);
            past_k = out.remove(1);
            let logits = out[0].to_plain_array_view::<f32>()?;
            let next = logits
                .iter()
                .enumerate()
                .fold((0, f32::NEG_INFINITY), |best, (i, v)| {
                    if *v > best.1 { (i, *v) } else { best }
                })
                .0;
            if next == EOS {
                break;
            }
            tokens.push(next);
            input = vec![next as i64];
        }
        Ok(tokens)
    }

    pub fn transcribe(&self, audio: &[f32]) -> Result<Vec<Note>> {
        let chunks = audio.len().div_ceil(SEGMENT);
        let mut tracker = Tracker::default();
        let mut notes = Notes::default();
        for index in 0..chunks {
            let seek = index as f64 * SEGMENT_SECONDS;
            let next = (index + 1 < chunks).then(|| (index + 1) as f64 * SEGMENT_SECONDS);
            notes.apply(tracker.boundary(seek, next));
            let prompt = if index > 0 {
                tie_prologue(&tracker.open_keys())
            } else {
                Vec::new()
            };
            let start = index * SEGMENT;
            let mut chunk = audio[start..(start + SEGMENT).min(audio.len())].to_vec();
            chunk.resize(SEGMENT, 0.0);
            eprintln!("muscriptor: chunk {}/{chunks}", index + 1);
            for id in self.generate(chunk, &prompt)? {
                notes.apply(tracker.feed(id));
            }
        }
        notes.apply(tracker.finish());
        Ok(trimmed(validated(notes.done)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulary_layout_matches_mt3() {
        assert_eq!(token(3), Token::Shift(0));
        assert_eq!(token(1003), Token::Shift(1000));
        assert_eq!(token(1004 + 60), Token::Pitch(60));
        assert_eq!(token(1133), Token::Velocity(1));
        assert_eq!(token(1134), Token::Tie);
        assert_eq!(token(1135 + 33), Token::Program(33));
        assert_eq!(token(1265 + 36), Token::Drum(36));
        assert_eq!(token(1), Token::Other);
    }

    #[test]
    fn tie_prologue_groups_pitches_by_program() {
        assert_eq!(
            tie_prologue(&[(0, 60), (0, 64), (33, 40)]),
            vec![
                PROGRAM,
                PITCH + 60,
                PITCH + 64,
                PROGRAM + 33,
                PITCH + 40,
                TIE
            ]
        );
    }

    #[test]
    fn notes_are_carried_across_a_tied_boundary() {
        let mut tracker = Tracker::default();
        let mut notes = Notes::default();
        notes.apply(tracker.boundary(0.0, Some(5.0)));
        for id in [
            TIE,
            PROGRAM,
            VELOCITY + 1,
            SHIFT + 100,
            PITCH + 60,
            SHIFT + 200,
            PITCH + 64,
        ] {
            notes.apply(tracker.feed(id));
        }
        notes.apply(tracker.boundary(5.0, None));
        assert_eq!(tracker.open_keys(), vec![(0, 60), (0, 64)]);
        for id in [PROGRAM, PITCH + 60, TIE, VELOCITY, SHIFT + 50, PITCH + 60] {
            notes.apply(tracker.feed(id));
        }
        notes.apply(tracker.finish());
        let notes = trimmed(validated(notes.done));
        assert_eq!(
            notes,
            vec![
                Note {
                    program: 0,
                    pitch: 60,
                    onset: 1.0,
                    offset: 5.5
                },
                Note {
                    program: 0,
                    pitch: 64,
                    onset: 2.0,
                    offset: 5.0
                },
            ]
        );
    }
}
