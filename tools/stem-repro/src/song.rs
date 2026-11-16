use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;

use crate::notes::{Note, read};
use crate::signal::{Frame, load, slice};

#[derive(Deserialize)]
pub struct Part {
    pub midi: String,
    pub track: Option<usize>,
    pub search_bars: Option<[f64; 2]>,
    pub rescore_bars: Option<[f64; 2]>,
}

#[derive(Deserialize)]
struct Config {
    bpm: f64,
    #[serde(default = "four")]
    beats_per_bar: f64,
    stems: String,
    premaster: String,
    master: Option<String>,
    stem_names: Vec<String>,
    #[serde(default)]
    sections: IndexMap<String, [f64; 2]>,
    #[serde(default)]
    parts: IndexMap<String, Part>,
}

fn four() -> f64 {
    4.0
}

pub struct Song {
    pub path: PathBuf,
    root: PathBuf,
    stems: String,
    pub premaster: PathBuf,
    pub master: Option<PathBuf>,
    pub bpm: f64,
    pub bar: f64,
    pub beat: f64,
    pub stem_names: Vec<String>,
    pub sections: IndexMap<String, [f64; 2]>,
    pub parts: IndexMap<String, Part>,
}

impl Song {
    pub fn open(config: &Path) -> Result<Self> {
        let path = config
            .canonicalize()
            .with_context(|| config.display().to_string())?;
        let root = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        let cfg: Config = serde_json::from_str(&std::fs::read_to_string(&path)?)
            .with_context(|| path.display().to_string())?;
        Ok(Self {
            premaster: root.join(&cfg.premaster),
            master: cfg.master.as_ref().map(|m| root.join(m)),
            stems: cfg.stems,
            bpm: cfg.bpm,
            bar: 60.0 / cfg.bpm * cfg.beats_per_bar,
            beat: 60.0 / cfg.bpm,
            stem_names: cfg.stem_names,
            sections: cfg.sections,
            parts: cfg.parts,
            path,
            root,
        })
    }

    #[must_use]
    pub fn file(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    #[must_use]
    pub fn stem_path(&self, name: &str) -> PathBuf {
        self.file(&self.stems.replace("{name}", name))
    }

    pub fn frames(&self) -> Result<usize> {
        if let Ok(reader) = hound::WavReader::open(&self.premaster) {
            return Ok(reader.duration() as usize);
        }
        Ok(audio::decode(&self.premaster)?.frames())
    }

    pub fn stem(&self, name: &str, frames: Option<usize>) -> Result<Vec<Frame>> {
        load(&self.stem_path(name), frames)
    }

    pub fn section<'a, T>(&self, x: &'a [T], name: &str) -> Result<&'a [T]> {
        let [b0, b1] = *self
            .sections
            .get(name)
            .with_context(|| format!("no section {name} in {}", self.path.display()))?;
        Ok(self.bars(x, b0, b1))
    }

    #[must_use]
    pub fn bars<'a, T>(&self, x: &'a [T], b0: f64, b1: f64) -> &'a [T] {
        slice(x, b0 * self.bar, b1 * self.bar)
    }

    pub fn part(&self, name: &str) -> Result<&Part> {
        self.parts
            .get(name)
            .with_context(|| format!("no part {name} in {}", self.path.display()))
    }

    pub fn part_notes(&self, name: &str) -> Result<Vec<Note>> {
        let part = self.part(name)?;
        read(&self.file(&part.midi), part.track)
    }

    #[must_use]
    pub fn notes_in(&self, notes: &[Note], b0: f64, b1: f64) -> Vec<Note> {
        notes
            .iter()
            .filter(|n| b0 * self.bar - 0.01 <= n.start && n.start < b1 * self.bar - 0.01)
            .copied()
            .collect()
    }
}
