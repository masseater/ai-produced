use std::collections::{HashMap, VecDeque};
use std::path::Path;

use anyhow::{Context, Result};
use midly::num::{u4, u7, u15, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use num_traits::ToPrimitive;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub time: f64,
    pub pitch: u8,
    pub velocity: Option<u8>,
}

struct TempoMap {
    ppq: f64,
    points: Vec<(u32, f64, f64)>,
}

impl TempoMap {
    fn new(smf: &Smf, ppq: f64) -> Self {
        let mut changes: Vec<(u32, usize, usize, u32)> = Vec::new();
        for (t, track) in smf.tracks.iter().enumerate() {
            let mut tick = 0u32;
            for (i, event) in track.iter().enumerate() {
                tick = tick.saturating_add(u32::from(event.delta));
                if let TrackEventKind::Meta(MetaMessage::Tempo(tempo)) = event.kind {
                    changes.push((tick, t, i, u32::from(tempo)));
                }
            }
        }
        changes.sort_unstable();
        let mut points = vec![(0u32, 0.0, 500_000.0)];
        for (tick, _, _, tempo) in changes {
            let (t0, s0, current) = points[points.len() - 1];
            points.push((
                tick,
                s0 + f64::from(tick - t0) * current / 1e6 / ppq,
                f64::from(tempo),
            ));
        }
        Self { ppq, points }
    }

    fn seconds(&self, tick: u32) -> f64 {
        let (t0, s0, tempo) = self
            .points
            .iter()
            .rev()
            .find(|p| p.0 <= tick)
            .copied()
            .unwrap_or(self.points[0]);
        s0 + f64::from(tick - t0) * tempo / 1e6 / self.ppq
    }
}

pub fn read(path: &Path, track: Option<usize>) -> Result<Vec<Note>> {
    let data = std::fs::read(path).with_context(|| path.display().to_string())?;
    let smf = Smf::parse(&data)?;
    let Timing::Metrical(ppq) = smf.header.timing else {
        anyhow::bail!("{}: timecode MIDI files are not supported", path.display());
    };
    let map = TempoMap::new(&smf, f64::from(ppq.as_int()));
    let tracks: Vec<&Vec<TrackEvent>> = match track {
        Some(t) => vec![
            smf.tracks
                .get(t)
                .with_context(|| format!("{}: no track {t}", path.display()))?,
        ],
        None => smf.tracks.iter().collect(),
    };
    let mut out = Vec::new();
    for events in tracks {
        let mut tick = 0u32;
        let mut held: HashMap<(u8, u8), VecDeque<(u32, u8)>> = HashMap::new();
        for event in events {
            tick = tick.saturating_add(u32::from(event.delta));
            let TrackEventKind::Midi { channel, message } = event.kind else {
                continue;
            };
            let channel = channel.as_int();
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    held.entry((channel, key.as_int()))
                        .or_default()
                        .push_back((tick, vel.as_int()));
                }
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    if let Some((start, velocity)) = held
                        .get_mut(&(channel, key.as_int()))
                        .and_then(VecDeque::pop_front)
                    {
                        out.push(Note {
                            start: map.seconds(start),
                            end: map.seconds(tick),
                            pitch: key.as_int(),
                            velocity,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    out.sort_by(|a, b| {
        (a.start, a.end, a.pitch, a.velocity)
            .partial_cmp(&(b.start, b.end, b.pitch, b.velocity))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

#[must_use]
pub fn shifted(notes: &[Note], shift: f64) -> Vec<Note> {
    notes
        .iter()
        .map(|n| Note {
            start: n.start + shift,
            end: n.end + shift,
            ..*n
        })
        .collect()
}

#[must_use]
pub fn events(notes: &[Note], t0: f64, t1: Option<f64>) -> Vec<Event> {
    let mut out = Vec::with_capacity(notes.len() * 2);
    for n in notes {
        let end = t1.map_or(n.end, |t1| n.end.min(t1));
        out.push(Event {
            time: (n.start - t0).max(0.0),
            pitch: n.pitch,
            velocity: Some(n.velocity),
        });
        out.push(Event {
            time: end - t0,
            pitch: n.pitch,
            velocity: None,
        });
    }
    out.sort_by(|a, b| a.time.total_cmp(&b.time));
    out
}

pub fn write(path: &Path, notes: &[Note], bpm: f64) -> Result<()> {
    let ticks_per_beat = 480u16;
    let tick = |s: f64| {
        (s * bpm / 60.0 * f64::from(ticks_per_beat))
            .round_ties_even()
            .to_i64()
            .with_context(|| format!("bad note time {s} s"))
    };
    let mut timed: Vec<(i64, u8, u8, u8)> = Vec::with_capacity(2 * notes.len());
    for n in notes {
        timed.push((tick(n.start)?, 1, n.pitch, n.velocity));
        timed.push((tick(n.end)?, 0, n.pitch, 0));
    }
    timed.sort_unstable();
    let mut track = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(midly::num::u24::new(
            (60e6 / bpm).round_ties_even().to_u32().context("tempo")?,
        ))),
    }];
    let mut now = 0i64;
    for (t, on, pitch, velocity) in timed {
        let key = u7::new(pitch);
        let vel = u7::new(velocity);
        let message = if on == 1 {
            MidiMessage::NoteOn { key, vel }
        } else {
            MidiMessage::NoteOff { key, vel }
        };
        track.push(TrackEvent {
            delta: u28::new(u32::try_from((t - now).max(0))?),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message,
            },
        });
        now = t;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(ticks_per_beat))),
        tracks: vec![track],
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    smf.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_notes_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.mid");
        let notes = [
            Note {
                start: 0.0,
                end: 0.5,
                pitch: 60,
                velocity: 100,
            },
            Note {
                start: 0.5,
                end: 1.0,
                pitch: 60,
                velocity: 90,
            },
        ];
        write(&path, &notes, 120.0).unwrap();
        let back = read(&path, None).unwrap();
        assert_eq!(back, notes);
        let e = events(&back, 0.25, Some(0.75));
        assert_eq!(e.len(), 4);
        assert!((e[0].time - 0.0).abs() < 1e-12);
        assert!((e[3].time - 0.5).abs() < 1e-12);
    }
}
