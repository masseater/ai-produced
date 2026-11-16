use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use num_traits::ToPrimitive;
use regex::Regex;
use serde::Deserialize;
use song::{moras, vowel};

const TPQ: i64 = 480;
const SIXTEENTH: i64 = TPQ / 4;
const BAR: i64 = TPQ * 4;
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const TRACKS: [(&str, u8, u8); 8] = [
    ("Vocal", 0, 54),
    ("Drums", 9, 0),
    ("Bass", 1, 38),
    ("Pad", 2, 89),
    ("Saw", 3, 81),
    ("Arp", 4, 80),
    ("Piano", 5, 0),
    ("Lead", 6, 81),
];
const DRUM_CHANNEL: u8 = 9;
const KICK: u8 = 36;
const RIM: u8 = 37;
const SNARE: u8 = 38;
const CLAP: u8 = 39;
const HAT: u8 = 42;
const OPEN_HAT: u8 = 46;
const CRASH: u8 = 49;
const TOM_LOW: u8 = 45;
const TOM_MID: u8 = 47;
const TOM_HIGH: u8 = 50;

#[derive(Parser)]
#[command(about = "Build MIDI and a vocal syllable table from a score TOML")]
struct Args {
    score: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Deserialize)]
struct Score {
    bpm: f64,
    key: String,
    bars: i64,
    lyrics: String,
    rules: Rules,
    #[serde(default)]
    motifs: HashMap<String, String>,
    sections: Vec<Section>,
}

#[derive(Deserialize)]
struct Rules {
    range: [String; 2],
    peak: String,
    #[serde(default)]
    peak_exceptions: usize,
    long_sixteenths: i64,
}

#[derive(Deserialize)]
struct Section {
    name: String,
    bars: usize,
    chords: String,
    mode: Option<String>,
    energy: Option<f64>,
    #[serde(default)]
    silent: Vec<String>,
    #[serde(default)]
    parts: HashMap<String, String>,
    #[serde(default)]
    vocal: Vec<VocalLine>,
}

#[derive(Deserialize)]
struct VocalLine {
    at: i64,
    notes: String,
    mode: Option<String>,
}

#[derive(Clone)]
struct Note {
    track: &'static str,
    start: i64,
    length: i64,
    pitch: u8,
    velocity: u8,
    lyric: String,
}

struct Syllable {
    section: String,
    line: usize,
    mora: String,
    vowel: char,
    closure: bool,
    start: i64,
    length: i64,
    pitch: u8,
    mode: String,
    text: String,
}

#[derive(Default)]
struct Song {
    notes: Vec<Note>,
    syllables: Vec<Syllable>,
    chords: Vec<(usize, String, String)>,
    markers: Vec<(i64, String)>,
    silences: Vec<(i64, i64)>,
    problems: Vec<String>,
}

struct LyricLine {
    text: String,
    reading: String,
}

fn pitch_class(letter: &str, accidental: Option<&str>) -> i64 {
    let base = match letter {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        _ => 11,
    };
    base + match accidental {
        Some("#") => 1,
        Some("b") => -1,
        _ => 0,
    }
}

fn pitch_of(name: &str) -> Result<u8> {
    let pattern = Regex::new(r"^([A-G])(#|b)?(-?\d)$")?;
    let found = pattern
        .captures(name)
        .ok_or_else(|| anyhow!("bad pitch {name}"))?;
    let octave: i64 = found[3].parse()?;
    let pitch = pitch_class(&found[1], found.get(2).map(|m| m.as_str())) + (octave + 1) * 12;
    u8::try_from(pitch).with_context(|| format!("bad pitch {name}"))
}

fn name_of(pitch: u8) -> String {
    format!(
        "{}{}",
        NAMES[usize::from(pitch % 12)],
        i64::from(pitch / 12) - 1
    )
}

fn chord_shape(quality: &str) -> Option<&'static [i64]> {
    match quality {
        "" => Some(&[0, 4, 7]),
        "m" => Some(&[0, 3, 7]),
        "sus4" => Some(&[0, 5, 7]),
        "m7" => Some(&[0, 3, 7, 10]),
        "maj7" => Some(&[0, 4, 7, 11]),
        "7" => Some(&[0, 4, 7, 10]),
        "add9" => Some(&[0, 4, 7, 14]),
        _ => None,
    }
}

fn pad_color(quality: &str) -> &str {
    match quality {
        "" => "add9",
        "m" => "m7",
        other => other,
    }
}

fn parse_chord(symbol: &str) -> Result<(i64, String)> {
    let pattern = Regex::new(r"^([A-G])(#|b)?(.*)$")?;
    let found = pattern
        .captures(symbol)
        .ok_or_else(|| anyhow!("bad chord {symbol}"))?;
    let root = pitch_class(&found[1], found.get(2).map(|m| m.as_str())).rem_euclid(12);
    let quality = found[3].to_string();
    if chord_shape(&quality).is_none() {
        bail!("bad chord {symbol}");
    }
    Ok((root, quality))
}

fn read_lyrics(path: &Path) -> Result<HashMap<String, Vec<LyricLine>>> {
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let mut sections: HashMap<String, Vec<LyricLine>> = HashMap::new();
    let mut current = String::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if let Some(header) = line.strip_prefix('#') {
            current = header.split('|').next().unwrap_or_default().to_string();
            sections.insert(current.clone(), Vec::new());
            continue;
        }
        let (lyric, reading) = line
            .split_once('|')
            .ok_or_else(|| anyhow!("lyric line without reading: {line}"))?;
        if !reading.trim().is_empty() {
            sections
                .get_mut(&current)
                .ok_or_else(|| anyhow!("lyric line before a section: {line}"))?
                .push(LyricLine {
                    text: lyric.to_string(),
                    reading: reading.to_string(),
                });
        }
    }
    Ok(sections)
}

fn expand(tokens: &str, motifs: &HashMap<String, String>) -> Result<Vec<String>> {
    let pattern = Regex::new(r"\$(\w+)")?;
    let mut missing = None;
    let expanded = pattern.replace_all(tokens, |found: &regex::Captures| {
        motifs.get(&found[1]).cloned().unwrap_or_else(|| {
            missing = Some(found[1].to_string());
            String::new()
        })
    });
    if let Some(name) = missing {
        bail!("unknown motif {name}");
    }
    Ok(expanded.split_whitespace().map(str::to_string).collect())
}

fn voice(pcs: &[i64], previous: Option<&[i64]>) -> Result<Vec<i64>> {
    let low = 52;
    let high = 76;
    let mut best: Option<(f64, Vec<i64>)> = None;
    for rotation in 0..pcs.len() {
        let order: Vec<i64> = pcs[rotation..]
            .iter()
            .chain(&pcs[..rotation])
            .copied()
            .collect();
        for base in low..low + 12 {
            if base % 12 != order[0] {
                continue;
            }
            let mut chord = vec![base];
            for pc in &order[1..] {
                let mut next = chord[chord.len() - 1] + 1;
                while next % 12 != *pc {
                    next += 1;
                }
                chord.push(next);
            }
            if chord[chord.len() - 1] > high {
                continue;
            }
            let cost = if let Some(previous) = previous {
                let mut sorted = chord.clone();
                sorted.sort_unstable();
                let mut before = previous.to_vec();
                before.sort_unstable();
                sorted
                    .iter()
                    .zip(&before)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<i64>()
                    .to_f64()
                    .context("voice leading cost")?
            } else {
                let sum = chord.iter().sum::<i64>().to_f64().context("chord sum")?;
                (sum / chord.len().to_f64().context("chord size")? - 64.0).abs()
            };
            if best.as_ref().is_none_or(|(lowest, _)| cost < *lowest) {
                best = Some((cost, chord));
            }
        }
    }
    Ok(best.map_or_default(|(_, chord)| chord))
}

fn bass_pitch(root: i64) -> i64 {
    28 + (root - 4).rem_euclid(12)
}

fn velocity(base: f64, energy: f64) -> Result<u8> {
    let scaled = base * (0.55 + 0.45 * energy);
    let clamped = scaled
        .to_i64()
        .ok_or_else(|| anyhow!("velocity {scaled} is not an integer"))?
        .clamp(1, 127);
    Ok(u8::try_from(clamped)?)
}

fn midi_pitch(pitch: i64) -> Result<u8> {
    Ok(u8::try_from(pitch.clamp(0, 127))?)
}

fn place_vocal(
    song: &mut Song,
    section: &Section,
    start_bar: i64,
    lyric_lines: &[LyricLine],
    motifs: &HashMap<String, String>,
) -> Result<()> {
    let entries = &section.vocal;
    if entries.len() != lyric_lines.len() {
        song.problems.push(format!(
            "{}: {} melody lines for {} lyric lines",
            section.name,
            entries.len(),
            lyric_lines.len()
        ));
        return Ok(());
    }
    for (index, (entry, lyric)) in entries.iter().zip(lyric_lines).enumerate() {
        let index = index + 1;
        let tokens = expand(&entry.notes, motifs)?;
        let sung_notes = tokens.iter().filter(|t| !t.starts_with('r')).count();
        let line_moras = moras(&lyric.reading);
        if sung_notes != line_moras.len() {
            song.problems.push(format!(
                "{} line {index}: {sung_notes} notes for {} morae ({})",
                section.name,
                line_moras.len(),
                lyric.reading
            ));
            continue;
        }
        let mut tick = (start_bar + entry.at) * BAR;
        let mode = entry
            .mode
            .clone()
            .or_else(|| section.mode.clone())
            .unwrap_or_else(|| "Standard".to_string());
        let mut mora_iter = line_moras.into_iter();
        for token in &tokens {
            if let Some(rest) = token.strip_prefix('r') {
                tick += rest.parse::<i64>()? * SIXTEENTH;
                continue;
            }
            let (length_text, pitch_text) = token
                .split_once(':')
                .ok_or_else(|| anyhow!("bad token {token}"))?;
            let length = length_text.parse::<i64>()? * SIXTEENTH;
            let mora = mora_iter.next().unwrap_or_default();
            let mut syllable = Syllable {
                section: section.name.clone(),
                line: index,
                mora: mora.clone(),
                vowel: 'Q',
                closure: true,
                start: tick,
                length,
                pitch: 0,
                mode: mode.clone(),
                text: lyric.text.clone(),
            };
            if pitch_text == "cl" {
                if mora != "っ" {
                    song.problems
                        .push(format!("{} line {index}: closure on {mora}", section.name));
                }
            } else {
                if mora == "っ" {
                    song.problems
                        .push(format!("{} line {index}: っ needs cl", section.name));
                }
                let pitch = pitch_of(pitch_text)?;
                syllable.vowel = vowel(&mora);
                syllable.closure = false;
                syllable.pitch = pitch;
                song.notes.push(Note {
                    track: "Vocal",
                    start: tick,
                    length,
                    pitch,
                    velocity: 96,
                    lyric: mora,
                });
            }
            song.syllables.push(syllable);
            tick += length;
        }
    }
    Ok(())
}

fn style_at(parts: &HashMap<String, String>, key: &str, index: usize) -> Result<String> {
    let spec: Vec<&str> = parts
        .get(key)
        .map_or("none", String::as_str)
        .split_whitespace()
        .collect();
    let mut expanded = Vec::new();
    for item in &spec {
        let (name, count) = item.split_once('*').unwrap_or((item, ""));
        let count = if count.is_empty() {
            1
        } else {
            count.parse::<usize>()?
        };
        expanded.extend(std::iter::repeat_n(name.to_string(), count));
    }
    let at = if spec.len() > 1 { index } else { 0 };
    expanded
        .get(at)
        .cloned()
        .ok_or_else(|| anyhow!("{key} pattern has no bar {index}"))
}

struct Bar<'a> {
    song: &'a mut Song,
    tick: i64,
    energy: f64,
}

impl Bar<'_> {
    fn v(&self, base: f64) -> Result<u8> {
        velocity(base, self.energy)
    }

    fn hit(&mut self, pos: f64, pitch: u8, base: f64) -> Result<()> {
        self.hit_for(pos, pitch, base, 1.0)
    }

    fn hit_for(&mut self, pos: f64, pitch: u8, base: f64, length: f64) -> Result<()> {
        let velocity = self.v(base)?;
        let sixteenths = |count: f64| (count * SIXTEENTH.to_f64()?).to_i64();
        let offset = sixteenths(pos).ok_or_else(|| anyhow!("bad drum position {pos}"))?;
        let length = sixteenths(length).ok_or_else(|| anyhow!("bad drum length {length}"))?;
        self.song.notes.push(Note {
            track: "Drums",
            start: self.tick + offset,
            length,
            pitch,
            velocity,
            lyric: String::new(),
        });
        Ok(())
    }

    fn add(
        &mut self,
        track: &'static str,
        pos: i64,
        length: i64,
        pitch: i64,
        base: f64,
    ) -> Result<()> {
        let velocity = self.v(base)?;
        self.song.notes.push(Note {
            track,
            start: self.tick + pos * SIXTEENTH,
            length: length * SIXTEENTH,
            pitch: midi_pitch(pitch)?,
            velocity,
            lyric: String::new(),
        });
        Ok(())
    }

    fn drums(&mut self, style: &str, bar_index: usize, bars: usize) -> Result<()> {
        let phrase_end = bar_index % 4 == 3;
        match style {
            "none" => {}
            "halftime_light" => {
                self.hit(8.0, RIM, 70.0)?;
                for pos in (0..16).step_by(2) {
                    self.hit(f64::from(pos), HAT, if pos % 4 == 0 { 60.0 } else { 45.0 })?;
                }
            }
            "halftime" => {
                for pos in [0.0, 10.0] {
                    self.hit(pos, KICK, 100.0)?;
                }
                self.hit(8.0, SNARE, 95.0)?;
                for pos in (0..16).step_by(2) {
                    self.hit(f64::from(pos), HAT, if pos % 4 == 0 { 70.0 } else { 50.0 })?;
                }
                if bar_index == 0 {
                    self.hit_for(0.0, CRASH, 80.0, 8.0)?;
                }
            }
            "heartbeat" => {
                self.hit(0.0, KICK, 70.0)?;
                self.hit(3.0, KICK, 55.0)?;
            }
            "build" => self.build(bar_index, bars)?,
            "chorus" | "last" => {
                for pos in [0.0, 6.0, 8.0] {
                    self.hit(pos, KICK, 115.0)?;
                }
                for pos in [4.0, 12.0] {
                    self.hit(pos, SNARE, 115.0)?;
                    self.hit(pos, CLAP, 85.0)?;
                }
                for pos in (0..16).step_by(2) {
                    let pitch = if pos == 14 { OPEN_HAT } else { HAT };
                    self.hit(
                        f64::from(pos),
                        pitch,
                        if pos % 4 == 0 { 90.0 } else { 70.0 },
                    )?;
                }
                let crash_every = if style == "last" { 2 } else { 4 };
                if bar_index.is_multiple_of(crash_every) {
                    self.hit_for(0.0, CRASH, 110.0, 8.0)?;
                }
                if phrase_end {
                    for (pos, tom) in (12..16).zip([TOM_HIGH, TOM_HIGH, TOM_MID, TOM_LOW]) {
                        self.hit(f64::from(pos), tom, 105.0)?;
                    }
                }
            }
            "four" => {
                for pos in [0.0, 4.0, 8.0, 12.0] {
                    self.hit(pos, KICK, 120.0)?;
                }
                for pos in [4.0, 12.0] {
                    self.hit(pos, CLAP, 110.0)?;
                }
                for pos in 0..16 {
                    let open = pos % 4 == 2;
                    self.hit(
                        f64::from(pos),
                        if open { OPEN_HAT } else { HAT },
                        if open { 80.0 } else { 50.0 },
                    )?;
                }
                if bar_index == 0 {
                    self.hit_for(0.0, CRASH, 110.0, 8.0)?;
                }
            }
            "accel" => self.accel(bar_index, bars)?,
            "hit" => {
                self.hit(0.0, KICK, 127.0)?;
                self.hit_for(0.0, CRASH, 120.0, 16.0)?;
            }
            "outro" => {
                if bar_index < 4 {
                    self.hit(0.0, KICK, 80.0)?;
                    self.hit(8.0, RIM, 60.0)?;
                }
            }
            other => bail!("unknown drum style {other}"),
        }
        Ok(())
    }

    fn build(&mut self, bar_index: usize, bars: usize) -> Result<()> {
        for pos in [0.0, 4.0, 8.0, 12.0] {
            self.hit(pos, KICK, 105.0)?;
        }
        let roll_bar = i64::try_from(bar_index)? - (i64::try_from(bars)? - 4);
        if roll_bar < 0 {
            for pos in [4.0, 12.0] {
                self.hit(pos, SNARE, 100.0)?;
            }
            for pos in (2..16).step_by(4) {
                self.hit(f64::from(pos), OPEN_HAT, 60.0)?;
            }
        } else {
            let step = match roll_bar {
                0 => 4,
                1 | 2 => 2,
                3 => 1,
                other => bail!("build has no roll bar {other}"),
            };
            for pos in (0..16).step_by(step) {
                let ramp = (roll_bar.to_f64().context("roll bar")? * 16.0 + f64::from(pos)) / 64.0;
                self.hit(f64::from(pos), SNARE, 60.0 + 60.0 * ramp)?;
            }
        }
        if bar_index == 0 {
            self.hit_for(0.0, CRASH, 90.0, 8.0)?;
        }
        Ok(())
    }

    fn accel(&mut self, bar_index: usize, bars: usize) -> Result<()> {
        let stage = bar_index / 2;
        if bar_index + 1 >= bars {
            return Ok(());
        }
        match stage {
            0 => {
                for pos in [0.0, 4.0, 8.0, 12.0] {
                    self.hit(pos, KICK, 100.0)?;
                }
            }
            1 => {
                for pos in (0..16).step_by(2) {
                    self.hit(f64::from(pos), KICK, 105.0)?;
                    self.hit(f64::from(pos), SNARE, 80.0 + 2.0 * f64::from(pos))?;
                }
            }
            2 => {
                for pos in 0..16 {
                    self.hit(f64::from(pos), SNARE, 85.0 + 2.0 * f64::from(pos))?;
                }
                for pos in [0.0, 4.0, 8.0, 12.0] {
                    self.hit(pos, KICK, 110.0)?;
                }
            }
            _ => {
                for half in 0..32 {
                    self.hit_for(f64::from(half) / 2.0, SNARE, 90.0 + f64::from(half), 0.5)?;
                }
                for pos in [0.0, 4.0, 8.0, 12.0] {
                    self.hit(pos, KICK, 115.0)?;
                }
            }
        }
        Ok(())
    }

    fn bass(&mut self, style: &str, root: i64, next_root: i64) -> Result<()> {
        let pitch = bass_pitch(root);
        match style {
            "none" => {}
            "sustain" => self.add("Bass", 0, 16, pitch, 100.0)?,
            "half" => {
                self.add("Bass", 0, 8, pitch, 100.0)?;
                self.add("Bass", 8, 8, pitch, 100.0)?;
            }
            "eighths" => {
                for pos in (0..16).step_by(2) {
                    let note = if pos == 6 || pos == 14 {
                        pitch + 12
                    } else {
                        pitch
                    };
                    self.add(
                        "Bass",
                        pos,
                        2,
                        note,
                        if pos % 4 == 0 { 110.0 } else { 90.0 },
                    )?;
                }
            }
            "offbeat" => {
                self.add("Bass", 0, 2, pitch, 115.0)?;
                for pos in [2, 6, 10, 14] {
                    self.add("Bass", pos, 2, pitch, 105.0)?;
                }
                self.add("Bass", 7, 1, pitch + 12, 80.0)?;
                self.add("Bass", 15, 1, pitch + 12, 80.0)?;
            }
            "build" => {
                for pos in (0..16).step_by(2) {
                    self.add(
                        "Bass",
                        pos,
                        2,
                        pitch,
                        90.0 + pos.to_f64().context("bass position")? * 2.0,
                    )?;
                }
            }
            "walk" => {
                self.add("Bass", 0, 6, pitch, 100.0)?;
                self.add("Bass", 6, 6, pitch, 100.0)?;
                let target = bass_pitch(next_root);
                let step = if target > pitch { 1 } else { -1 };
                let last = if (target - pitch).abs() > 2 {
                    pitch + step * 2
                } else {
                    pitch
                };
                self.add("Bass", 12, 4, last, 100.0)?;
            }
            "stop" => self.add("Bass", 0, 4, pitch, 120.0)?,
            other => bail!("unknown bass style {other}"),
        }
        Ok(())
    }

    fn comp(&mut self, part: &'static str, style: &str, voicing: &[i64]) -> Result<()> {
        let last = voicing[voicing.len() - 1];
        match style {
            "none" => {}
            "sustain" => {
                for p in voicing {
                    self.add(part, 0, 16, *p, 70.0)?;
                }
            }
            "stabs" => {
                for pos in [0, 3, 6, 8, 11, 14] {
                    for p in voicing {
                        self.add(part, pos, 2, p + 12, 65.0)?;
                    }
                }
            }
            "offstabs" => {
                for pos in [2, 6, 10, 14] {
                    for p in voicing {
                        self.add(part, pos, 1, p + 12, 70.0)?;
                    }
                }
            }
            "broken" => {
                let order = [voicing[0], voicing[2], voicing[1], last];
                for (i, p) in (0_i64..).zip(order.iter().chain(&order)) {
                    self.add(part, i * 2, 2, *p, if i % 2 == 0 { 80.0 } else { 70.0 })?;
                }
            }
            "quarters" => {
                for (i, p) in (0_i64..).zip(&[voicing[0], voicing[1], voicing[2], last]) {
                    self.add(part, i * 4, 4, *p, 60.0)?;
                }
            }
            "hit" => {
                for p in voicing {
                    self.add(part, 0, 4, p + 12, 110.0)?;
                }
            }
            "arp" => {
                let mut cycle: Vec<i64> = voicing.iter().map(|p| p + 12).collect();
                cycle.sort_unstable();
                cycle.dedup();
                let mut pattern = cycle.clone();
                if cycle.len() > 2 {
                    pattern.extend(cycle[1..cycle.len() - 1].iter().rev());
                }
                for (pos, &note) in (0..16).zip(pattern.iter().cycle()) {
                    self.add(part, pos, 1, note, if pos % 4 == 0 { 70.0 } else { 55.0 })?;
                }
            }
            other => bail!("unknown {part} style {other}"),
        }
        Ok(())
    }
}

fn hook_shape(tokens: &str) -> Result<Vec<(i64, i64, u8)>> {
    let mut tick = 0;
    let mut out = Vec::new();
    for token in tokens.split_whitespace() {
        if let Some(rest) = token.strip_prefix('r') {
            tick += rest.parse::<i64>()? * SIXTEENTH;
            continue;
        }
        let (length_text, pitch_text) = token
            .split_once(':')
            .ok_or_else(|| anyhow!("bad token {token}"))?;
        let length = length_text.parse::<i64>()? * SIXTEENTH;
        if pitch_text != "cl" {
            out.push((tick, length, pitch_of(pitch_text)?));
        }
        tick += length;
    }
    Ok(out)
}

fn silence_of(spec: &str, position: usize) -> Result<(i64, i64)> {
    let pattern = Regex::new(r"^(\d+):(\d+)-(\d+)$")?;
    let found = pattern
        .captures(spec)
        .ok_or_else(|| anyhow!("bad silence {spec}"))?;
    let bar = (i64::try_from(position)? + found[1].parse::<i64>()?) * BAR;
    Ok((
        bar + found[2].parse::<i64>()? * SIXTEENTH,
        bar + found[3].parse::<i64>()? * SIXTEENTH,
    ))
}

fn build(score: &Score, base_dir: &Path) -> Result<Song> {
    let mut song = Song::default();
    let lyrics = read_lyrics(&base_dir.join(&score.lyrics))?;
    let mut voicing: Option<Vec<i64>> = None;
    let mut all_chords: Vec<(&Section, usize, &str)> = Vec::new();
    for section in &score.sections {
        let symbols: Vec<&str> = section.chords.split_whitespace().collect();
        if symbols.len() != section.bars {
            song.problems.push(format!(
                "{}: {} chords for {} bars",
                section.name,
                symbols.len(),
                section.bars
            ));
        }
        all_chords.extend(
            symbols
                .into_iter()
                .enumerate()
                .map(|(i, symbol)| (section, i, symbol)),
        );
    }
    let mut bar = 0;
    for (position, (section, index, symbol)) in all_chords.iter().enumerate() {
        let (section, index, symbol) = (*section, *index, *symbol);
        let start_bar = i64::try_from(position)?;
        if index == 0 {
            song.markers.push((start_bar * BAR, section.name.clone()));
            for spec in &section.silent {
                let silence = silence_of(spec, position)?;
                song.silences.push(silence);
            }
            let lines = lyrics.get(&section.name).map_or(&[][..], Vec::as_slice);
            place_vocal(&mut song, section, start_bar, lines, &score.motifs)?;
        }
        let parts = &section.parts;
        song.chords
            .push((position + 1, section.name.clone(), symbol.to_string()));
        if symbol == "NC" {
            continue;
        }
        let (root, quality) = parse_chord(symbol)?;
        let shape = chord_shape(pad_color(&quality)).unwrap_or(&[0, 4, 7]);
        let pcs: Vec<i64> = shape.iter().map(|i| (root + i).rem_euclid(12)).collect();
        let chord = voice(&pcs, voicing.as_deref())?;
        let following = all_chords.get(position + 1).map_or(symbol, |next| next.2);
        let next_root = if following == "NC" {
            root
        } else {
            parse_chord(following)?.0
        };
        let mut current = Bar {
            song: &mut song,
            tick: start_bar * BAR,
            energy: section.energy.unwrap_or(0.5),
        };
        current.drums(&style_at(parts, "drums", index)?, index, section.bars)?;
        current.bass(&style_at(parts, "bass", index)?, root, next_root)?;
        for part in ["Pad", "Saw", "Arp", "Piano"] {
            current.comp(part, &style_at(parts, &part.to_lowercase(), index)?, &chord)?;
        }
        if style_at(parts, "lead", index)? == "hook" {
            let hook = score
                .motifs
                .get("HOOK")
                .ok_or_else(|| anyhow!("lead hook needs a HOOK motif"))?;
            for (token_start, length, pitch) in hook_shape(hook)? {
                current.song.notes.push(Note {
                    track: "Lead",
                    start: current.tick + token_start,
                    length,
                    pitch: pitch + 12,
                    velocity: 70,
                    lyric: String::new(),
                });
            }
        }
        voicing = Some(chord);
        bar = position + 1;
    }
    double_vocal(&mut song, score);
    apply_silence(&mut song);
    check(&mut song, score, bar)?;
    Ok(song)
}

fn double_vocal(song: &mut Song, score: &Score) {
    let doubled: Vec<&str> = score
        .sections
        .iter()
        .filter(|s| s.parts.get("lead").is_some_and(|lead| lead == "double"))
        .map(|s| s.name.as_str())
        .collect();
    let extra: Vec<Note> = song
        .syllables
        .iter()
        .filter(|s| !s.closure && doubled.contains(&s.section.as_str()))
        .map(|s| Note {
            track: "Lead",
            start: s.start,
            length: s.length,
            pitch: s.pitch + 12,
            velocity: 65,
            lyric: String::new(),
        })
        .collect();
    song.notes.extend(extra);
}

fn apply_silence(song: &mut Song) {
    let silences = song.silences.clone();
    song.notes.retain_mut(|note| {
        let end = note.start + note.length;
        for (lo, hi) in &silences {
            if *lo <= note.start && note.start < *hi {
                return false;
            }
            if note.start < *lo && *lo < end {
                note.length = lo - note.start;
            }
        }
        true
    });
}

fn check(song: &mut Song, score: &Score, bars: usize) -> Result<()> {
    let rules = &score.rules;
    if i64::try_from(bars)? != score.bars {
        song.problems
            .push(format!("song has {bars} bars, expected {}", score.bars));
    }
    let low = pitch_of(&rules.range[0])?;
    let high = pitch_of(&rules.range[1])?;
    let peak = pitch_of(&rules.peak)?;
    let over_peak = song
        .syllables
        .iter()
        .filter(|s| !s.closure && s.pitch > peak)
        .count();
    if over_peak > rules.peak_exceptions {
        song.problems
            .push(format!("{over_peak} notes above {}", rules.peak));
    }
    let mut problems = Vec::new();
    for s in &song.syllables {
        if !s.closure && !(low..=high).contains(&s.pitch) {
            problems.push(format!(
                "{} line {}: {} out of range",
                s.section,
                s.line,
                name_of(s.pitch)
            ));
        }
    }
    let long_ticks = rules.long_sixteenths * SIXTEENTH;
    for (i, s) in song.syllables.iter().enumerate() {
        let hook_tail = i > 0 && song.syllables[i - 1].mora == "か" && s.mora == "い";
        if !s.closure && s.length >= long_ticks && !"aoe".contains(s.vowel) && !hook_tail {
            problems.push(format!(
                "{} line {}: long note on {}",
                s.section, s.line, s.mora
            ));
        }
    }
    for pair in song.syllables.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.start + a.length > b.start {
            problems.push(format!(
                "{} line {}: overlap at {}",
                a.section, a.line, a.mora
            ));
        }
        if (a.section != b.section || a.line != b.line) && a.start + a.length == b.start {
            problems.push(format!(
                "{} line {}: no breath before line",
                b.section, b.line
            ));
        }
    }
    song.problems.extend(problems);
    Ok(())
}

fn key_signature(key: &str) -> Result<(i8, bool)> {
    let minor = key.ends_with('m');
    let tonic = key.trim_end_matches('m');
    let major = [
        "Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#",
    ];
    let minor_keys = [
        "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#",
    ];
    let table = if minor { &minor_keys } else { &major };
    let index = table
        .iter()
        .position(|name| *name == tonic)
        .ok_or_else(|| anyhow!("bad key {key}"))?;
    Ok((i8::try_from(index)? - 7, minor))
}

type Events<'a> = Vec<(i64, u8, TrackEventKind<'a>)>;

fn delta(ticks: i64) -> Result<u28> {
    Ok(u28::new(
        u32::try_from(ticks).context("events out of order")?,
    ))
}

fn sequence<'a>(
    mut head: Vec<TrackEventKind<'a>>,
    mut events: Events<'a>,
) -> Result<Vec<TrackEvent<'a>>> {
    events.sort_by_key(|(at, order, _)| (*at, *order));
    let mut track: Vec<TrackEvent<'a>> = head
        .drain(..)
        .map(|kind| TrackEvent {
            delta: u28::new(0),
            kind,
        })
        .collect();
    let mut now = 0;
    for (at, _, kind) in events {
        track.push(TrackEvent {
            delta: delta(at - now)?,
            kind,
        });
        now = at;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(track)
}

fn write_midi(song: &Song, score: &Score, path: &Path, start: i64, end: Option<i64>) -> Result<()> {
    let inside = |t: i64| start <= t && end.is_none_or(|end| t < end);
    let (sharps, minor) = key_signature(&score.key)?;
    let tempo = (60_000_000.0 / score.bpm)
        .round()
        .to_u32()
        .ok_or_else(|| anyhow!("bad bpm {}", score.bpm))?;
    let conductor_head = vec![
        TrackEventKind::Meta(MetaMessage::Tempo(u24::new(tempo))),
        TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
        TrackEventKind::Meta(MetaMessage::KeySignature(sharps, minor)),
    ];
    let markers: Events = song
        .markers
        .iter()
        .filter(|(t, _)| inside(*t))
        .map(|(t, name)| {
            (
                t - start,
                1,
                TrackEventKind::Meta(MetaMessage::Marker(name.as_bytes())),
            )
        })
        .collect();
    let mut tracks = vec![sequence(conductor_head, markers)?];
    for (name, channel, program) in TRACKS {
        let channel_number = u4::new(channel);
        let mut head = vec![TrackEventKind::Meta(MetaMessage::TrackName(
            name.as_bytes(),
        ))];
        if channel != DRUM_CHANNEL {
            head.push(TrackEventKind::Midi {
                channel: channel_number,
                message: MidiMessage::ProgramChange {
                    program: u7::new(program),
                },
            });
        }
        let mut events: Events = Vec::new();
        for note in song
            .notes
            .iter()
            .filter(|n| n.track == name && inside(n.start))
        {
            let at = note.start - start;
            if !note.lyric.is_empty() {
                events.push((
                    at,
                    1,
                    TrackEventKind::Meta(MetaMessage::Lyric(note.lyric.as_bytes())),
                ));
            }
            events.push((
                at,
                2,
                TrackEventKind::Midi {
                    channel: channel_number,
                    message: MidiMessage::NoteOn {
                        key: u7::new(note.pitch),
                        vel: u7::new(note.velocity),
                    },
                },
            ));
            events.push((
                at + (note.length - 10).max(1),
                0,
                TrackEventKind::Midi {
                    channel: channel_number,
                    message: MidiMessage::NoteOff {
                        key: u7::new(note.pitch),
                        vel: u7::new(0),
                    },
                },
            ));
        }
        tracks.push(sequence(head, events)?);
    }
    let smf = Smf {
        header: Header::new(
            Format::Parallel,
            Timing::Metrical(u15::new(u16::try_from(TPQ)?)),
        ),
        tracks,
    };
    smf.save(path).with_context(|| path.display().to_string())
}

fn beat_text(tick: i64) -> Result<String> {
    let beat =
        ((tick % BAR).to_f64().context("tick")? / TPQ.to_f64().context("TPQ")? + 1.0) * 1000.0;
    let rounded = beat.round() / 1000.0;
    if rounded.fract() == 0.0 {
        Ok(format!("{rounded:.1}"))
    } else {
        Ok(format!("{rounded}"))
    }
}

fn write_tables(song: &Song, out_dir: &Path) -> Result<()> {
    let tsv = |path: PathBuf| {
        csv::WriterBuilder::new()
            .delimiter(b'\t')
            .terminator(csv::Terminator::Any(b'\n'))
            .from_path(path)
    };
    let mut vocal = tsv(out_dir.join("vocal.tsv"))?;
    vocal.write_record([
        "section", "line", "text", "mora", "vowel", "kind", "bar", "beat", "tick", "ticks", "note",
        "midi", "mode",
    ])?;
    for s in &song.syllables {
        let (kind, note, midi) = if s.closure {
            ("closure", String::new(), String::new())
        } else {
            ("note", name_of(s.pitch), s.pitch.to_string())
        };
        vocal.write_record([
            s.section.clone(),
            s.line.to_string(),
            s.text.clone(),
            s.mora.clone(),
            s.vowel.to_string(),
            kind.to_string(),
            (s.start / BAR + 1).to_string(),
            beat_text(s.start)?,
            s.start.to_string(),
            s.length.to_string(),
            note,
            midi,
            s.mode.clone(),
        ])?;
    }
    vocal.flush()?;
    let mut chords = tsv(out_dir.join("chords.tsv"))?;
    chords.write_record(["bar", "section", "chord"])?;
    for (bar, section, chord) in &song.chords {
        chords.write_record([bar.to_string(), section.clone(), chord.clone()])?;
    }
    chords.flush()?;
    Ok(())
}

fn run(args: &Args) -> Result<bool> {
    let text = fs::read_to_string(&args.score).with_context(|| args.score.display().to_string())?;
    let score: Score = toml::from_str(&text)?;
    let base_dir = args.score.parent().unwrap_or_else(|| Path::new("."));
    let song = build(&score, base_dir)?;
    if !song.problems.is_empty() {
        eprintln!("{}", song.problems.join("\n"));
        return Ok(false);
    }
    let sections_dir = args.out.join("sections");
    fs::create_dir_all(&sections_dir)?;
    write_midi(&song, &score, &args.out.join("full.mid"), 0, None)?;
    let mut bounds: Vec<(i64, &str)> = song
        .markers
        .iter()
        .map(|(t, name)| (*t, name.as_str()))
        .collect();
    bounds.push((score.bars * BAR, ""));
    for (number, pair) in bounds.windows(2).enumerate() {
        let ((start, name), (end, _)) = (pair[0], pair[1]);
        let path = sections_dir.join(format!("{:02}-{name}.mid", number + 1));
        write_midi(&song, &score, &path, start, Some(end))?;
    }
    write_tables(&song, &args.out)?;
    let pitches: Vec<u8> = song
        .syllables
        .iter()
        .filter(|s| !s.closure)
        .map(|s| s.pitch)
        .collect();
    println!(
        "bars={} vocal_notes={} range={}-{}",
        score.bars,
        pitches.len(),
        name_of(pitches.iter().copied().min().unwrap_or_default()),
        name_of(pitches.iter().copied().max().unwrap_or_default())
    );
    Ok(true)
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    Ok(if run(&args)? {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
