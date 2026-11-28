use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::shared::mora::moras;
use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};

const PPQ: i64 = 480;
const SIXTEENTH: i64 = PPQ / 4;
const HOOK: [&str; 6] = ["も", "う", "い", "っ", "か", "い"];

#[derive(Parser)]
#[command(about = "vocal.tsv と score.toml から、16分音符単位の歌詞タイミングJSONを作る")]
struct Args {
    score: PathBuf,
    vocal: PathBuf,
    out: PathBuf,
}

#[derive(Deserialize)]
struct Score {
    bpm: toml::Value,
    lyrics: String,
    sections: Vec<ScoreSection>,
}

#[derive(Deserialize)]
struct ScoreSection {
    name: String,
    bars: i64,
}

#[derive(Deserialize)]
struct Row {
    section: String,
    line: usize,
    mora: String,
    kind: String,
    tick: i64,
    ticks: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Song {
    bpm: toml::Value,
    beats_per_bar: i64,
    bars: i64,
    hooks: usize,
    sections: Vec<Section>,
    lines: Vec<Line>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Section {
    name: String,
    start_bar: i64,
    bars: i64,
}

#[derive(Serialize)]
struct Line {
    section: String,
    index: usize,
    text: String,
    note: String,
    words: Vec<Word>,
    morae: Vec<Mora>,
}

#[derive(Serialize)]
struct Word {
    text: String,
    morae: Vec<usize>,
}

#[derive(Serialize)]
struct Mora {
    text: String,
    kind: String,
    start: i64,
    length: i64,
    hook: usize,
}

struct Lyric {
    text: String,
    reading: String,
    note: String,
}

fn read_lyrics(path: &Path) -> Result<HashMap<(String, usize), Lyric>> {
    let note = Regex::new(r"（[^）]*）")?;
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let mut lines = HashMap::new();
    let mut section = String::new();
    let mut index = 0;
    for raw in text.lines() {
        if let Some(header) = raw.strip_prefix('#') {
            section = header.split('|').next().unwrap_or_default().to_string();
            index = 0;
            continue;
        }
        let (lyric, reading) = raw
            .split_once('|')
            .ok_or_else(|| anyhow!("lyric line without reading: {raw}"))?;
        if reading.is_empty() {
            continue;
        }
        index += 1;
        let found = note.find(lyric).map_or("", |m| {
            m.as_str().trim_start_matches('（').trim_end_matches('）')
        });
        lines.insert(
            (section.clone(), index),
            Lyric {
                text: note.replace_all(lyric, "").trim().to_string(),
                reading: reading.to_string(),
                note: found.to_string(),
            },
        );
    }
    Ok(lines)
}

fn kana_tail(chunk: &str) -> String {
    let tail: Vec<char> = chunk
        .chars()
        .rev()
        .take_while(|c| ('ぁ'..='ゖ').contains(c) || *c == 'ー')
        .collect();
    tail.into_iter().rev().collect()
}

fn group_readings(chunks: &[&str], readings: &[&str]) -> Option<Vec<String>> {
    let mut rest = readings.iter();
    let mut remaining = readings.len();
    let mut grouped = Vec::new();
    for chunk in chunks {
        let tail = kana_tail(chunk);
        let mut taken = String::new();
        while remaining > 0 && (taken.is_empty() || !taken.ends_with(&tail)) {
            taken.push_str(rest.next()?);
            remaining -= 1;
        }
        grouped.push(taken);
    }
    (remaining == 0 && grouped.iter().all(|g| !g.is_empty())).then_some(grouped)
}

fn words_of(lyric: &Lyric, sung: usize) -> Vec<Word> {
    let chunks: Vec<&str> = lyric.text.split_whitespace().collect();
    let readings: Vec<&str> = lyric.reading.split_whitespace().collect();
    let whole = || {
        vec![Word {
            text: lyric.text.clone(),
            morae: (0..sung).collect(),
        }]
    };
    let Some(grouped) = group_readings(&chunks, &readings) else {
        return whole();
    };
    let counts: Vec<usize> = grouped.iter().map(|r| moras(r).len()).collect();
    if counts.iter().sum::<usize>() != sung {
        return whole();
    }
    let mut start = 0;
    chunks
        .iter()
        .zip(counts)
        .map(|(chunk, count)| {
            let word = Word {
                text: (*chunk).to_string(),
                morae: (start..start + count).collect(),
            };
            start += count;
            word
        })
        .collect()
}

fn mark_hooks(rows: &[Row]) -> Vec<usize> {
    let mut marks = vec![0; rows.len()];
    let mut hook = 0;
    let mut i = 0;
    while i < rows.len() {
        let matches = rows.len() - i >= HOOK.len()
            && rows[i..i + HOOK.len()]
                .iter()
                .zip(HOOK)
                .all(|(row, mora)| row.mora == mora);
        if matches {
            hook += 1;
            marks[i..i + HOOK.len()].fill(hook);
            i += HOOK.len();
        } else {
            i += 1;
        }
    }
    marks
}

fn build(score_path: &Path, vocal_path: &Path) -> Result<Song> {
    let text = fs::read_to_string(score_path).with_context(|| score_path.display().to_string())?;
    let score: Score = toml::from_str(&text)?;
    let base = score_path.parent().unwrap_or_else(|| Path::new("."));
    let lyrics = read_lyrics(&base.join(&score.lyrics))?;
    let rows: Vec<Row> = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_path(vocal_path)
        .with_context(|| vocal_path.display().to_string())?
        .deserialize()
        .collect::<Result<_, _>>()?;
    for row in &rows {
        if row.tick % SIXTEENTH != 0 || row.ticks % SIXTEENTH != 0 {
            bail!(
                "16分の格子から外れたノート: {} {} {} tick={} ticks={}",
                row.section,
                row.line,
                row.mora,
                row.tick,
                row.ticks
            );
        }
    }
    let marks = mark_hooks(&rows);
    let mut bar = 0;
    let sections = score
        .sections
        .iter()
        .map(|s| {
            let section = Section {
                name: s.name.clone(),
                start_bar: bar,
                bars: s.bars,
            };
            bar += s.bars;
            section
        })
        .collect();
    let mut order: Vec<(String, usize)> = Vec::new();
    let mut grouped: HashMap<(String, usize), Vec<Mora>> = HashMap::new();
    for (row, hook) in rows.iter().zip(&marks) {
        let key = (row.section.clone(), row.line);
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(Mora {
            text: row.mora.clone(),
            kind: row.kind.clone(),
            start: row.tick / SIXTEENTH,
            length: row.ticks / SIXTEENTH,
            hook: *hook,
        });
    }
    let mut lines = Vec::new();
    for key in order {
        let lyric = lyrics
            .get(&key)
            .ok_or_else(|| anyhow!("no lyric for {} line {}", key.0, key.1))?;
        let morae = grouped.remove(&key).unwrap_or_default();
        lines.push(Line {
            words: words_of(lyric, morae.len()),
            section: key.0,
            index: key.1,
            text: lyric.text.clone(),
            note: lyric.note.clone(),
            morae,
        });
    }
    Ok(Song {
        bpm: score.bpm,
        beats_per_bar: 4,
        bars: bar,
        hooks: marks.iter().copied().max().unwrap_or(0),
        sections,
        lines,
    })
}

pub fn run() -> Result<()> {
    execute(&Args::parse())
}

fn execute(args: &Args) -> Result<()> {
    let song = build(&args.score, &args.vocal)?;
    let mut text = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut text, formatter);
    song.serialize(&mut serializer)?;
    text.push(b'\n');
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, text)?;
    println!(
        "{}行、フック{}回、{}小節 → {}",
        song.lines.len(),
        song.hooks,
        song.bars,
        args.out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::test_repository::{repository, song};

    #[test]
    fn reproduces_the_video_timing() {
        let out = tempfile::tempdir().expect("tempdir");
        let json = out.path().join("timing.json");
        let composition = song().join("02-composition");
        let args = Args::parse_from([
            "lyric-timing".as_ref(),
            composition.join("score.toml").as_os_str(),
            composition.join("generated/vocal.tsv").as_os_str(),
            json.as_os_str(),
        ]);
        execute(&args).expect("lyric-timing");
        let committed =
            repository().join("apps/video/src/pages/mou-ikkai/generated/mou-ikkai.json");
        assert_eq!(
            fs::read(json).expect("json"),
            fs::read(committed).expect("committed")
        );
    }
}
