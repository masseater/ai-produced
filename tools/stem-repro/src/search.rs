use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};

use crate::features::dist;
use crate::notes::{Event, events};
use crate::signal::mono;
use crate::song::Song;
use crate::surge::{Surge, install, patches};

const MARK: &str = "stem-repro-result\t";
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_TASKS: usize = 200;

type Scores = IndexMap<String, [f64; 2]>;

struct Excerpt {
    events: Vec<Event>,
    reference: Vec<f64>,
    duration: f64,
}

fn bars_of(song: &Song, part: &str, key: &str) -> Result<[f64; 2]> {
    let p = song.part(part)?;
    match key {
        "search_bars" => p.search_bars,
        _ => p.rescore_bars,
    }
    .with_context(|| format!("part {part} has no {key}"))
}

fn excerpt(song: &Song, part: &str, key: &str) -> Result<Excerpt> {
    let [b0, b1] = bars_of(song, part, key)?;
    let (t0, t1) = (b0 * song.bar, b1 * song.bar);
    Ok(Excerpt {
        events: events(
            &song.notes_in(&song.part_notes(part)?, b0, b1),
            t0,
            Some(t1),
        ),
        reference: mono(song.bars(&song.stem(part, None)?, b0, b1)),
        duration: t1 - t0,
    })
}

fn score(v: &[f64; 2]) -> f64 {
    v[0] + 0.5 * v[1]
}

pub fn worker(config: &Path, key: &str) -> Result<()> {
    let song = Song::open(config)?;
    let jobs: IndexMap<String, Excerpt> = song
        .parts
        .keys()
        .map(|p| Ok((p.clone(), excerpt(&song, p, key)?)))
        .collect::<Result<_>>()?;
    let mut synth = Surge::new(&install()?)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Some((patch, parts)) = line.split_once('\t') else {
            continue;
        };
        synth.load(Path::new(patch), &[])?;
        let mut scores = Map::new();
        for part in parts.split(',').filter(|p| !p.is_empty()) {
            let job = &jobs[part];
            let value = match synth.render_mono(&job.events, job.duration, job.reference.len())? {
                Some(m) => {
                    let (s, e) = dist(&m, &job.reference)?;
                    [s, e]
                }
                None => [99.0, 99.0],
            };
            scores.insert(part.to_string(), json!(value));
        }
        writeln!(stdout, "{MARK}{}", Value::Object(scores))?;
        stdout.flush()?;
    }
    Ok(())
}

struct Process {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    tasks: usize,
}

impl Process {
    fn spawn(config: &Path, key: &str) -> Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg(config)
            .arg("search-worker")
            .arg("--key")
            .arg(key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().context("worker has no stdin")?;
        let stdout = child.stdout.take().context("worker has no stdout")?;
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix(MARK)
                    && sender.send(rest.to_string()).is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            lines,
            tasks: 0,
        })
    }

    fn ask(&mut self, patch: &Path, parts: &[String]) -> Option<Scores> {
        writeln!(self.stdin, "{}\t{}", patch.display(), parts.join(",")).ok()?;
        self.stdin.flush().ok()?;
        self.tasks += 1;
        match self.lines.recv_timeout(TIMEOUT) {
            Ok(line) => serde_json::from_str(&line).ok(),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

type Row = (PathBuf, Vec<String>);

fn run_stage(
    config: &Path,
    key: &str,
    rows: Vec<Row>,
    workers: usize,
) -> (Vec<(PathBuf, Scores)>, Vec<PathBuf>) {
    let queue = Arc::new(Mutex::new(
        rows.into_iter().enumerate().collect::<VecDeque<_>>(),
    ));
    let results = Arc::new(Mutex::new(Vec::new()));
    let bad = Arc::new(Mutex::new(Vec::new()));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let (queue, results, bad) = (queue.clone(), results.clone(), bad.clone());
            scope.spawn(move || {
                let mut process: Option<Process> = None;
                while let Some((index, (patch, parts))) =
                    queue.lock().ok().and_then(|mut q| q.pop_front())
                {
                    if process.as_ref().is_some_and(|p| p.tasks >= MAX_TASKS)
                        && let Some(p) = process.take()
                    {
                        p.stop();
                    }
                    if process.is_none() {
                        process = Process::spawn(config, key).ok();
                    }
                    let answer = process.as_mut().and_then(|p| p.ask(&patch, &parts));
                    if let Some(scores) = answer {
                        if let Ok(mut r) = results.lock() {
                            r.push((index, patch, scores));
                        }
                    } else {
                        {
                            if let Some(p) = process.take() {
                                p.stop();
                            }
                            if let Ok(mut b) = bad.lock() {
                                b.push((index, patch));
                            }
                        }
                    }
                }
                if let Some(p) = process {
                    p.stop();
                }
            });
        }
    });
    let mut results = std::mem::take(&mut *results.lock().expect("workers finished"));
    results.sort_by_key(|r| r.0);
    let mut bad = std::mem::take(&mut *bad.lock().expect("workers finished"));
    bad.sort_by_key(|b| b.0);
    (
        results.into_iter().map(|(_, p, s)| (p, s)).collect(),
        bad.into_iter().map(|(_, p)| p).collect(),
    )
}

pub fn run(song: &Song, out: &Path, workers: usize, top: usize) -> Result<()> {
    let started = Instant::now();
    let install = install()?;
    let files = patches(&install.patches)?;
    let parts: Vec<String> = song.parts.keys().cloned().collect();
    let rows: Vec<Row> = files.iter().map(|f| (f.clone(), parts.clone())).collect();
    let (first, bad1) = run_stage(&song.path, "search_bars", rows, workers);
    println!(
        "stage1 {} patches {} s, crashed {}",
        first.len(),
        started.elapsed().as_secs(),
        bad1.len()
    );
    let mut need: IndexMap<PathBuf, Vec<String>> = IndexMap::new();
    for part in &parts {
        let mut ranked: Vec<&(PathBuf, Scores)> =
            first.iter().filter(|(_, s)| s.contains_key(part)).collect();
        ranked.sort_by(|a, b| score(&a.1[part]).total_cmp(&score(&b.1[part])));
        for (file, _) in ranked.into_iter().take(top) {
            need.entry(file.clone()).or_default().push(part.clone());
        }
    }
    let (second, bad2) = run_stage(
        &song.path,
        "rescore_bars",
        need.into_iter().collect(),
        workers,
    );
    println!("stage2 {} {} s", second.len(), started.elapsed().as_secs());
    let mut final_ranking = Map::new();
    for part in &parts {
        let mut ranked: Vec<(PathBuf, [f64; 2])> = second
            .iter()
            .filter_map(|(f, s)| s.get(part).map(|v| (f.clone(), *v)))
            .collect();
        ranked.sort_by(|a, b| score(&a.1).total_cmp(&score(&b.1)));
        println!("== {part}");
        for (file, [spec, env]) in ranked.iter().take(5) {
            let relative = file.strip_prefix(&install.patches).unwrap_or(file);
            println!("  {spec:.2} {env:.2} {}", relative.display());
        }
        final_ranking.insert(
            part.clone(),
            json!(
                ranked
                    .iter()
                    .map(|(f, [s, e])| json!({"f": f, "spec": s, "env": e}))
                    .collect::<Vec<_>>()
            ),
        );
    }
    std::fs::create_dir_all(out)?;
    let crashed: Vec<&PathBuf> = bad1.iter().chain(&bad2).collect();
    std::fs::write(
        out.join("result.json"),
        serde_json::to_string_pretty(&json!({"final": final_ranking, "crashed": crashed}))?,
    )?;
    Ok(())
}
