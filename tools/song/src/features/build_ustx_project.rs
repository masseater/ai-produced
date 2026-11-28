use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

const TPQ: i64 = 480;
const BAR: i64 = TPQ * 4;
const PHONEMIZER: &str = "OpenUtau.Core.DiffSinger.DiffSingerJapanesePhonemizer";
const CLOSURE_LYRIC: &str = "っ";
const DEFAULT_MODE: &str = "Standard";
const PORTAMENTO: i64 = 40;
const BREATH_LYRIC: &str = "AP";
const CURVES: [(&str, i64); 5] = [
    ("brec", 0),
    ("tenc", 0),
    ("voic", 100),
    ("velc", 100),
    ("dyn", 0),
];
const CURVE_RAMP: i64 = 120;

#[derive(Parser)]
#[command(
    about = "Build an OpenUtau USTX project from vocal.tsv (score-midi output). Closure rows become a note sung as っ, which the DiffSinger Japanese phonemizer turns into the cl phoneme."
)]
struct Args {
    #[arg(help = "vocal.tsv")]
    vocal: PathBuf,
    #[arg(long, help = "score.toml, for bpm and key")]
    score: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, help = "singer id, as `openutau-render singers` prints it")]
    singer: Option<String>,
    #[arg(long, default_value = PHONEMIZER, help = "phonemizer type name")]
    phonemizer: String,
    #[arg(
        long,
        help = "tuning TOML: per-section mode, curves (brec, tenc, voic, velc, dyn), portamento, vibrato, staccato; breath insertion"
    )]
    tuning: Option<PathBuf>,
    #[arg(
        long,
        help = "voice colors of the singer, comma separated, as `openutau-render singers` prints them; maps the mode column to clr"
    )]
    colors: Option<String>,
}

#[derive(Deserialize)]
struct Score {
    bpm: toml::Value,
    key: String,
    sections: Vec<ScoreSection>,
}

#[derive(Deserialize)]
struct ScoreSection {
    name: String,
    bars: i64,
}

#[derive(Deserialize, Default)]
struct Tuning {
    vibrato: Option<VibratoSettings>,
    breath: Option<Breath>,
    #[serde(default)]
    section: std::collections::HashMap<String, SectionStyle>,
}

#[derive(Deserialize, Clone, Copy)]
struct VibratoSettings {
    min_ticks: i64,
    length: i64,
    period: i64,
    depth: i64,
    fade_in: i64,
    fade_out: i64,
}

#[derive(Deserialize)]
struct Breath {
    min_rest: i64,
    ticks: i64,
    gap: i64,
    sections: Vec<String>,
}

#[derive(Deserialize, Default)]
struct SectionStyle {
    mode: Option<String>,
    portamento: Option<i64>,
    vibrato: Option<bool>,
    staccato: Option<f64>,
    brec: Option<i64>,
    tenc: Option<i64>,
    voic: Option<i64>,
    velc: Option<i64>,
    #[serde(rename = "dyn")]
    dynamics: Option<i64>,
}

impl SectionStyle {
    fn curve(&self, abbr: &str) -> Option<i64> {
        match abbr {
            "brec" => self.brec,
            "tenc" => self.tenc,
            "voic" => self.voic,
            "velc" => self.velc,
            _ => self.dynamics,
        }
    }
}

#[derive(Deserialize)]
struct Row {
    section: String,
    mora: String,
    kind: String,
    tick: i64,
    ticks: i64,
    midi: Option<i64>,
    mode: String,
}

#[derive(Serialize, Clone)]
struct PitchPoint {
    x: i64,
    y: i64,
    shape: &'static str,
}

#[derive(Serialize, Clone)]
struct Pitch {
    data: Vec<PitchPoint>,
    snap_first: bool,
}

#[derive(Serialize, Clone)]
struct Vibrato {
    length: i64,
    period: i64,
    depth: i64,
    #[serde(rename = "in")]
    fade_in: i64,
    #[serde(rename = "out")]
    fade_out: i64,
    shift: i64,
    drift: i64,
    vol_link: i64,
}

#[derive(Serialize, Clone)]
struct Expression {
    index: i64,
    abbr: &'static str,
    value: usize,
}

#[derive(Serialize, Clone)]
struct Note {
    position: i64,
    duration: i64,
    tone: i64,
    lyric: String,
    pitch: Pitch,
    vibrato: Vibrato,
    tuning: i64,
    phoneme_expressions: Vec<Expression>,
    phoneme_overrides: Vec<()>,
    #[serde(skip)]
    section: String,
}

#[derive(Serialize)]
struct Curve {
    xs: Vec<i64>,
    ys: Vec<i64>,
    abbr: &'static str,
}

#[derive(Serialize)]
struct Track {
    #[serde(skip_serializing_if = "Option::is_none")]
    singer: Option<String>,
    phonemizer: String,
    renderer_settings: Empty,
    #[serde(rename = "track_name")]
    name: &'static str,
    #[serde(rename = "track_color")]
    color: &'static str,
    mute: bool,
    solo: bool,
    volume: i64,
    pan: i64,
    #[serde(rename = "track_expressions")]
    expressions: Vec<()>,
    voice_color_names: Vec<String>,
}

#[derive(Serialize)]
struct Empty {}

#[derive(Serialize)]
struct TimeSignature {
    bar_position: i64,
    beat_per_bar: i64,
    beat_unit: i64,
}

#[derive(Serialize)]
struct Tempo {
    position: i64,
    bpm: toml::Value,
}

#[derive(Serialize)]
struct VoicePart {
    duration: i64,
    name: &'static str,
    comment: &'static str,
    track_no: i64,
    position: i64,
    notes: Vec<Note>,
    curves: Vec<Curve>,
    masked_curves: Vec<()>,
}

#[derive(Serialize)]
struct Project {
    name: String,
    comment: &'static str,
    output_dir: &'static str,
    cache_dir: &'static str,
    ustx_version: &'static str,
    key: i64,
    time_signatures: Vec<TimeSignature>,
    tempos: Vec<Tempo>,
    tracks: Vec<Track>,
    voice_parts: Vec<VoicePart>,
    wave_parts: Vec<()>,
}

fn key_of(name: &str) -> Result<i64> {
    let mut chars = name.chars();
    let tonic = match chars.next() {
        Some('C') => 0,
        Some('D') => 2,
        Some('E') => 4,
        Some('F') => 5,
        Some('G') => 7,
        Some('A') => 9,
        Some('B') => 11,
        _ => bail!("bad key {name}"),
    } + match chars.next() {
        Some('#') => 1,
        Some('b') => -1,
        _ => 0,
    };
    Ok(if name.ends_with('m') {
        (tonic + 3i64).rem_euclid(12)
    } else {
        tonic.rem_euclid(12)
    })
}

fn portamento(ms: i64) -> Pitch {
    Pitch {
        data: vec![
            PitchPoint {
                x: -ms,
                y: 0,
                shape: "io",
            },
            PitchPoint {
                x: ms,
                y: 0,
                shape: "io",
            },
        ],
        snap_first: true,
    }
}

fn vibrato(length: i64, settings: Option<VibratoSettings>) -> Vibrato {
    match settings {
        Some(s) if length >= s.min_ticks => Vibrato {
            length: s.length,
            period: s.period,
            depth: s.depth,
            fade_in: s.fade_in,
            fade_out: s.fade_out,
            shift: 0,
            drift: 0,
            vol_link: 0,
        },
        _ => Vibrato {
            length: 0,
            period: 175,
            depth: 25,
            fade_in: 10,
            fade_out: 10,
            shift: 0,
            drift: 0,
            vol_link: 0,
        },
    }
}

fn end_of(note: &Note) -> i64 {
    note.position + note.duration
}

fn build_notes(rows: &[Row], colors: &[String], tuning: &Tuning) -> Result<Vec<Note>> {
    let fallback = SectionStyle::default();
    let mut notes: Vec<Note> = Vec::new();
    let mut missing = BTreeSet::new();
    for row in rows {
        let start = row.tick;
        let (tone, lyric) = if row.kind == "closure" {
            let previous = notes
                .last()
                .filter(|previous| end_of(previous) == start)
                .ok_or_else(|| anyhow!("closure without a preceding note at tick {start}"))?;
            (previous.tone, CLOSURE_LYRIC.to_string())
        } else {
            let tone = row
                .midi
                .ok_or_else(|| anyhow!("note without midi at tick {start}"))?;
            let lyric = if row.mora == "を" { "お" } else { &row.mora };
            (tone, lyric.to_string())
        };
        if notes
            .last()
            .is_some_and(|previous| end_of(previous) > start)
        {
            bail!("overlapping notes at tick {start}");
        }
        let style = tuning.section.get(&row.section).unwrap_or(&fallback);
        let mode = style.mode.clone().unwrap_or_else(|| row.mode.clone());
        let mut expressions = Vec::new();
        if !colors.is_empty() {
            let color = if mode == DEFAULT_MODE && colors.iter().any(String::is_empty) {
                ""
            } else {
                mode.as_str()
            };
            match colors.iter().position(|c| c == color) {
                Some(value) => expressions.push(Expression {
                    index: 0,
                    abbr: "clr",
                    value,
                }),
                None => {
                    missing.insert(mode.clone());
                }
            }
        }
        let settings = if style.vibrato.unwrap_or(true) {
            tuning.vibrato
        } else {
            None
        };
        notes.push(Note {
            position: start,
            duration: row.ticks,
            tone,
            lyric,
            pitch: portamento(style.portamento.unwrap_or(PORTAMENTO)),
            vibrato: vibrato(row.ticks, settings),
            tuning: 0,
            phoneme_expressions: expressions,
            phoneme_overrides: Vec::new(),
            section: row.section.clone(),
        });
    }
    if !missing.is_empty() {
        bail!(
            "modes not among --colors: {}",
            missing.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(notes)
}

fn shorten(notes: &mut [Note], tuning: &Tuning) -> Result<()> {
    let lyrics: Vec<String> = notes.iter().map(|n| n.lyric.clone()).collect();
    for (i, note) in notes.iter_mut().enumerate() {
        let ratio = tuning.section.get(&note.section).and_then(|s| s.staccato);
        let tied = lyrics
            .get(i + 1)
            .is_some_and(|after| after == CLOSURE_LYRIC);
        if let Some(ratio) = ratio
            && ratio != 0.0
            && note.lyric != CLOSURE_LYRIC
            && !tied
        {
            let steps = (note.duration.to_f64().context("note duration")? * ratio / 30.0)
                .round_ties_even()
                .to_i64()
                .ok_or_else(|| anyhow!("bad staccato ratio {ratio}"))?;
            let scaled = steps * 30;
            note.duration = scaled.max(60);
        }
    }
    Ok(())
}

fn add_breaths(notes: Vec<Note>, tuning: &Tuning) -> Vec<Note> {
    let Some(settings) = &tuning.breath else {
        return notes;
    };
    let mut out: Vec<Note> = Vec::new();
    for note in notes {
        if let Some(previous) = out.last() {
            let rest = note.position - end_of(previous);
            if rest >= settings.min_rest && settings.sections.contains(&note.section) {
                let length = settings.ticks.min(rest - settings.gap);
                out.push(Note {
                    position: note.position - length,
                    duration: length,
                    lyric: BREATH_LYRIC.to_string(),
                    vibrato: vibrato(0, None),
                    ..note.clone()
                });
            }
        }
        out.push(note);
    }
    out
}

fn build_curves(score: &Score, tuning: &Tuning) -> Vec<Curve> {
    let fallback = SectionStyle::default();
    let mut bar = 0;
    let spans: Vec<(&str, i64, i64)> = score
        .sections
        .iter()
        .map(|s| {
            let span = (s.name.as_str(), bar * BAR, (bar + s.bars) * BAR);
            bar += s.bars;
            span
        })
        .collect();
    let mut curves = Vec::new();
    for (abbr, default) in CURVES {
        let values: Vec<(i64, i64, i64)> = spans
            .iter()
            .map(|(name, start, end)| {
                let style = tuning.section.get(*name).unwrap_or(&fallback);
                (*start, *end, style.curve(abbr).unwrap_or(default))
            })
            .collect();
        if values.iter().all(|(_, _, value)| *value == default) {
            continue;
        }
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for (start, end, value) in values {
            xs.extend([start, end - CURVE_RAMP]);
            ys.extend([value, value]);
        }
        curves.push(Curve { xs, ys, abbr });
    }
    curves
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    Ok(toml::from_str(&text)?)
}

pub fn run() -> Result<()> {
    execute(&Args::parse())
}

fn execute(args: &Args) -> Result<()> {
    let score: Score = read_toml(&args.score)?;
    let colors: Vec<String> = args
        .colors
        .as_deref()
        .map_or_default(|c| c.split(',').map(str::to_string).collect());
    let tuning: Tuning = match &args.tuning {
        Some(path) => read_toml(path)?,
        None => Tuning::default(),
    };
    let rows: Vec<Row> = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_path(&args.vocal)
        .with_context(|| args.vocal.display().to_string())?
        .deserialize()
        .collect::<Result<_, _>>()?;
    let mut notes = build_notes(&rows, &colors, &tuning)?;
    shorten(&mut notes, &tuning)?;
    let notes = add_breaths(notes, &tuning);
    let curves = build_curves(&score, &tuning);
    let end = notes.last().map_or(0, end_of);
    let closures = notes.iter().filter(|n| n.lyric == CLOSURE_LYRIC).count();
    let breaths = notes.iter().filter(|n| n.lyric == BREATH_LYRIC).count();
    let count = notes.len();
    let duration = (end + BAR - 1).div_euclid(BAR) * BAR;
    let project = Project {
        name: args
            .out
            .file_stem()
            .map_or_default(|s| s.to_string_lossy().into_owned()),
        comment: "",
        output_dir: "Vocal",
        cache_dir: "UCache",
        ustx_version: "0.10",
        key: key_of(&score.key)?,
        time_signatures: vec![TimeSignature {
            bar_position: 0,
            beat_per_bar: 4,
            beat_unit: 4,
        }],
        tempos: vec![Tempo {
            position: 0,
            bpm: score.bpm.clone(),
        }],
        tracks: vec![Track {
            singer: args.singer.clone(),
            phonemizer: args.phonemizer.clone(),
            renderer_settings: Empty {},
            name: "Vocal",
            color: "Blue",
            mute: false,
            solo: false,
            volume: 0,
            pan: 0,
            expressions: Vec::new(),
            voice_color_names: if colors.is_empty() {
                vec![String::new()]
            } else {
                colors
            },
        }],
        voice_parts: vec![VoicePart {
            duration,
            name: "Vocal",
            comment: "",
            track_no: 0,
            position: 0,
            notes,
            curves,
            masked_curves: Vec::new(),
        }],
        wave_parts: Vec::new(),
    };
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, serde_saphyr::to_string(&project)?)?;
    println!(
        "{}: {count} notes ({closures} closures, {breaths} breaths), {} bars",
        args.out.display(),
        duration / BAR
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::test_repository::song;

    #[test]
    fn reproduces_the_untuned_project() {
        let out = tempfile::tempdir().expect("tempdir");
        let ustx = out.path().join("mou-ikkai.ustx");
        let args = Args::parse_from([
            "ustx-from-vocal".as_ref(),
            song()
                .join("02-composition/generated/vocal.tsv")
                .as_os_str(),
            "--score".as_ref(),
            song().join("02-composition/score.toml").as_os_str(),
            "--out".as_ref(),
            ustx.as_os_str(),
        ]);
        execute(&args).expect("ustx-from-vocal");
        let parse = |path: &Path| -> serde_json::Value {
            serde_saphyr::from_str(&fs::read_to_string(path).expect("ustx")).expect("yaml")
        };
        assert_eq!(parse(&ustx), parse(&song().join("03-vocal/mou-ikkai.ustx")));
    }
}
