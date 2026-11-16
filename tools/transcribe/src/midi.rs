use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use num_traits::ToPrimitive;

pub const DRUM_PROGRAM: u16 = 128;

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub program: u16,
    pub pitch: u8,
    pub onset: f64,
    pub offset: f64,
}

impl Note {
    #[must_use]
    pub fn is_drum(&self) -> bool {
        self.program == DRUM_PROGRAM
    }
}

#[derive(Clone, Debug)]
pub struct ScoredNote {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
}

pub struct Instrument {
    pub program: u8,
    pub is_drum: bool,
    pub notes: Vec<ScoredNote>,
}

struct TempoMap {
    changes: Vec<(u32, f64)>,
    ppq: f64,
}

impl TempoMap {
    fn seconds(&self, tick: u32) -> f64 {
        let mut seconds = 0.0;
        let mut last_tick = 0;
        let mut per_tick = 0.5 / self.ppq;
        for &(at, tempo) in &self.changes {
            if at >= tick {
                break;
            }
            seconds += f64::from(at - last_tick) * per_tick;
            last_tick = at;
            per_tick = tempo / 1e6 / self.ppq;
        }
        seconds + f64::from(tick - last_tick) * per_tick
    }
}

pub fn read(data: &[u8]) -> Result<Vec<Instrument>> {
    let smf = Smf::parse(data)?;
    let Timing::Metrical(ppq) = smf.header.timing else {
        return Err(anyhow!("timecode MIDI files are not supported"));
    };
    let mut changes = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u32;
        for event in track {
            tick = tick.saturating_add(u32::from(event.delta));
            if let TrackEventKind::Meta(MetaMessage::Tempo(tempo)) = event.kind {
                changes.push((tick, f64::from(u32::from(tempo))));
            }
        }
    }
    changes.sort_by_key(|(tick, _)| *tick);
    let map = TempoMap {
        changes,
        ppq: f64::from(ppq.as_int()),
    };
    let mut instruments: Vec<Instrument> = Vec::new();
    let mut index: HashMap<(u8, u8, usize), usize> = HashMap::new();
    for (track_index, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0u32;
        let mut programs = [0u8; 16];
        let mut open: HashMap<(u8, u8), Vec<u32>> = HashMap::new();
        for event in track {
            tick = tick.saturating_add(u32::from(event.delta));
            let TrackEventKind::Midi { channel, message } = event.kind else {
                continue;
            };
            let channel = channel.as_int();
            let (key, on) = match message {
                MidiMessage::ProgramChange { program } => {
                    programs[channel as usize] = program.as_int();
                    continue;
                }
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => (key.as_int(), true),
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    (key.as_int(), false)
                }
                _ => continue,
            };
            let starts = open.entry((channel, key)).or_default();
            if on {
                starts.push(tick);
                continue;
            }
            if starts.is_empty() {
                continue;
            }
            let start = starts.remove(0);
            let program = programs[channel as usize];
            let slot = *index
                .entry((program, channel, track_index))
                .or_insert_with(|| {
                    instruments.push(Instrument {
                        program,
                        is_drum: channel == 9,
                        notes: Vec::new(),
                    });
                    instruments.len() - 1
                });
            instruments[slot].notes.push(ScoredNote {
                start: map.seconds(start),
                end: map.seconds(tick),
                pitch: key,
            });
        }
    }
    Ok(instruments)
}

pub const TICKS_PER_BEAT: u16 = 480;
const TEMPO: u32 = 500_000;
const DRUM_BEATS: f64 = 1.0 / 32.0;

fn tick_of(beats: f64) -> Result<u32> {
    (beats * f64::from(TICKS_PER_BEAT))
        .round_ties_even()
        .max(0.0)
        .to_u32()
        .with_context(|| format!("bad note time {beats} beats"))
}

struct Event {
    beats: f64,
    is_drum: bool,
    program: u16,
    on: bool,
    pitch: u8,
}

fn sorted_events(notes: &[Note]) -> Vec<Event> {
    let seconds_per_beat = f64::from(TEMPO) / 1e6;
    let mut events: Vec<Event> = Vec::new();
    for note in notes {
        let onset = note.onset / seconds_per_beat;
        events.push(Event {
            beats: onset,
            is_drum: note.is_drum(),
            program: note.program,
            on: true,
            pitch: note.pitch,
        });
        events.push(Event {
            beats: if note.is_drum() {
                onset + DRUM_BEATS
            } else {
                note.offset / seconds_per_beat
            },
            is_drum: note.is_drum(),
            program: note.program,
            on: false,
            pitch: note.pitch,
        });
    }
    events.sort_by(|a, b| {
        a.beats
            .total_cmp(&b.beats)
            .then(a.is_drum.cmp(&b.is_drum))
            .then(a.program.cmp(&b.program))
            .then(a.on.cmp(&b.on))
            .then(a.pitch.cmp(&b.pitch))
    });
    events
}

fn track_end() -> TrackEvent<'static> {
    TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    }
}

fn tempo_event() -> TrackEvent<'static> {
    TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(TEMPO))),
    }
}

pub fn write(notes: &[Note], name: fn(u16) -> String) -> Result<Vec<u8>> {
    let mut order: Vec<u16> = Vec::new();
    let mut channels: HashMap<u16, u8> = HashMap::new();
    let mut free: Vec<u8> = (0..9).chain(10..16).rev().collect();
    let mut messages: HashMap<u16, Vec<(u32, MidiMessage)>> = HashMap::new();
    for event in sorted_events(notes) {
        let key = event.program;
        channels.entry(key).or_insert_with(|| {
            order.push(key);
            if key == DRUM_PROGRAM {
                9
            } else {
                free.pop().unwrap_or(15)
            }
        });
        let pitch = u7::new(event.pitch);
        let message = if event.on {
            MidiMessage::NoteOn {
                key: pitch,
                vel: u7::new(100),
            }
        } else {
            MidiMessage::NoteOff {
                key: pitch,
                vel: u7::new(0),
            }
        };
        messages
            .entry(key)
            .or_default()
            .push((tick_of(event.beats)?, message));
    }
    let names: Vec<String> = order.iter().map(|key| name(*key)).collect();
    let mut tracks = vec![vec![tempo_event(), track_end()]];
    for (key, name) in order.iter().zip(&names) {
        let channel = u4::new(channels[key]);
        let program = if *key == DRUM_PROGRAM {
            0
        } else {
            u8::try_from(*key)?
        };
        let mut track = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(name.as_bytes())),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::ProgramChange {
                        program: u7::new(program),
                    },
                },
            },
            tempo_event(),
        ];
        let mut last = 0;
        for (tick, message) in &messages[key] {
            track.push(TrackEvent {
                delta: u28::new(tick - last),
                kind: TrackEventKind::Midi {
                    channel,
                    message: *message,
                },
            });
            last = *tick;
        }
        track.push(track_end());
        tracks.push(track);
    }
    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(TICKS_PER_BEAT))),
        tracks,
    };
    let mut out = Vec::new();
    smf.write_std(&mut out)?;
    Ok(out)
}
