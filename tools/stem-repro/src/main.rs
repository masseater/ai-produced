mod design;
mod eq;
mod features;
mod interstem;
mod kick;
mod mastering;
mod note_shape;
mod notes;
mod notes_filter;
mod optim;
mod optimize;
mod place;
mod resynth;
mod search;
mod shape;
mod signal;
mod song;
mod stems;
mod surge;
mod timbre;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::{PrettyFormatter, Serializer};
use song::Song;

#[derive(Parser)]
#[command(
    about = "Reproduce a reference song from its published stems and MIDI with Surge XT, and measure how each stem and the mix differ"
)]
struct Args {
    #[arg(help = "song.json")]
    config: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(clap::Args)]
struct InterstemArgs {
    #[arg(long)]
    section: String,
    #[arg(long)]
    kick: String,
    #[arg(long)]
    bass: String,
    #[arg(long)]
    repro: Option<PathBuf>,
}

#[derive(clap::Args)]
struct MasteringArgs {
    #[arg(long)]
    bus: bool,
}

#[derive(clap::Args)]
struct FitBusMasterArgs {
    #[arg(long)]
    stems_dir: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(clap::Args)]
struct KickFitArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    section: String,
    #[arg(long, default_value_t = 0.42)]
    length: f64,
    #[arg(long, default_value_t = 16)]
    hits: usize,
    #[arg(long, default_value_t = 60)]
    maxiter: u64,
    #[arg(long, default_value_t = 3)]
    seed: u64,
}

#[derive(clap::Args)]
struct KickTrackArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    section: String,
    #[arg(long)]
    params: PathBuf,
    #[arg(long, default_value_t = 0.42)]
    length: f64,
    #[arg(long, default_value_t = 16)]
    hits: usize,
    #[arg(long)]
    out: PathBuf,
}

#[derive(clap::Args)]
struct SearchArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 4)]
    workers: usize,
    #[arg(long, default_value_t = 50)]
    top: usize,
}

#[derive(clap::Args)]
struct SearchWorkerArgs {
    #[arg(long)]
    key: String,
}

#[derive(clap::Args)]
struct OptimizeArgs {
    #[arg(long)]
    part: String,
    #[arg(long)]
    patch: PathBuf,
    #[arg(long, default_value_t = 25)]
    maxiter: u64,
    #[arg(long, default_value_t = 10)]
    popsize: usize,
}

#[derive(clap::Args)]
struct DesignArgs {
    #[arg(long)]
    part: String,
    #[arg(long, default_value = "Init Saw")]
    template: String,
    #[arg(long, default_value = "lp24", value_parser = design::FILTERS.map(|f| f.0))]
    filter: String,
    #[arg(long, num_args = 2, required = true)]
    bars: Vec<f64>,
    #[arg(long, default_value_t = 150)]
    iters: u64,
    #[arg(long)]
    noise: bool,
    #[arg(long)]
    midi: Option<PathBuf>,
}

#[derive(clap::Args)]
struct RenderArgs {
    #[arg(long)]
    part: String,
    #[arg(long)]
    params: PathBuf,
    #[arg(long)]
    midi: Option<PathBuf>,
    #[arg(long)]
    out: PathBuf,
}

#[derive(clap::Args)]
struct FilterNotesArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    midi: PathBuf,
    #[arg(long, num_args = 2)]
    bars: Option<Vec<f64>>,
    #[arg(long, default_value_t = -30.0, allow_negative_numbers = true)]
    threshold: f64,
}

#[derive(clap::Args)]
struct EditNotesArgs {
    #[arg(long)]
    midi: PathBuf,
    #[arg(long)]
    track: Option<usize>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    splice: Option<PathBuf>,
    #[arg(long, num_args = 1..)]
    splice_bars: Vec<f64>,
    #[arg(long)]
    grid: Option<u32>,
    #[arg(long)]
    fixed: bool,
}

#[derive(clap::Args)]
struct PlaceArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    raw: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(clap::Args)]
struct ShapeArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    raw: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    bars_only: bool,
}

#[derive(clap::Args)]
struct NoteShapeArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    raw: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    part: Option<String>,
    #[arg(long)]
    midi: Option<PathBuf>,
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    shift_ms: f64,
}

#[derive(clap::Args)]
struct TimbreArgs {
    #[arg(long)]
    stem: String,
    #[arg(long)]
    repro: PathBuf,
    #[arg(long, num_args = 2, required = true)]
    bars: Vec<f64>,
    #[arg(long)]
    part: Option<String>,
    #[arg(long)]
    midi: Option<PathBuf>,
}

#[derive(clap::Args)]
struct ResynthArgs {
    #[arg(long)]
    stem: String,
    #[arg(long, default_value_t = 512)]
    fft: usize,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Subcommand)]
enum Cmd {
    Stems,
    Interstem(InterstemArgs),
    Mastering(MasteringArgs),
    FitBusMaster(FitBusMasterArgs),
    KickFit(KickFitArgs),
    KickTrack(KickTrackArgs),
    Search(SearchArgs),
    #[command(hide = true)]
    SearchWorker(SearchWorkerArgs),
    Optimize(OptimizeArgs),
    Design(DesignArgs),
    Render(RenderArgs),
    FilterNotes(FilterNotesArgs),
    EditNotes(EditNotesArgs),
    Place(PlaceArgs),
    Shape(ShapeArgs),
    NoteShape(NoteShapeArgs),
    Timbre(TimbreArgs),
    Resynth(ResynthArgs),
}

fn pair(values: &[f64]) -> [f64; 2] {
    [values[0], values[1]]
}

fn print(value: &Value) -> Result<()> {
    let mut out = Vec::new();
    let mut serializer = Serializer::with_formatter(&mut out, PrettyFormatter::with_indent(b" "));
    value.serialize(&mut serializer)?;
    println!("{}", String::from_utf8(out)?);
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Cmd::SearchWorker(a) = &args.command {
        return search::worker(&args.config, &a.key);
    }
    let song = Song::open(&args.config)?;
    let value = match args.command {
        Cmd::Stems => stems::run(&song)?,
        Cmd::Interstem(a) => {
            interstem::run(&song, &a.section, &a.kick, &a.bass, a.repro.as_deref())?
        }
        Cmd::Mastering(a) => mastering::run(&song, a.bus)?,
        Cmd::FitBusMaster(a) => mastering::run_fit(&song, &a.stems_dir, &a.out)?,
        Cmd::KickFit(a) => kick::run_fit(
            &song, &a.stem, &a.section, a.length, a.hits, a.maxiter, a.seed,
        )?,
        Cmd::KickTrack(a) => kick::run_track(
            &song, &a.stem, &a.section, &a.params, a.length, a.hits, &a.out,
        )?,
        Cmd::Search(a) => return search::run(&song, &a.out, a.workers, a.top),
        Cmd::SearchWorker(_) => return Ok(()),
        Cmd::Optimize(a) => optimize::run(&song, &a.part, &a.patch, a.maxiter, a.popsize)?,
        Cmd::Design(a) => design::run(
            &song,
            &design::Design {
                part: &a.part,
                template: &a.template,
                filter: &a.filter,
                bars: pair(&a.bars),
                iterations: a.iters,
                noise: a.noise,
                midi: a.midi.as_deref(),
            },
        )?,
        Cmd::Render(a) => design::run_render(&song, &a.part, &a.params, a.midi.as_deref(), &a.out)?,
        Cmd::FilterNotes(a) => notes_filter::run(
            &song,
            &a.stem,
            &a.midi,
            a.bars.as_deref().map(pair),
            a.threshold,
        )?,
        Cmd::EditNotes(a) => notes_filter::run_edit(
            &song,
            &notes_filter::Edit {
                midi: &a.midi,
                track: a.track,
                out: &a.out,
                splice: a.splice.as_deref(),
                splice_bars: &a.splice_bars,
                grid: a.grid,
                fixed: a.fixed,
            },
        )?,
        Cmd::Place(a) => place::run(&song, &a.stem, &a.raw, &a.out)?,
        Cmd::Shape(a) => shape::run(&song, &a.stem, &a.raw, &a.out, a.bars_only)?,
        Cmd::NoteShape(a) => note_shape::run(
            &song,
            &a.stem,
            &a.raw,
            &a.out,
            a.part.as_deref(),
            a.midi.as_deref(),
            a.shift_ms,
        )?,
        Cmd::Timbre(a) => timbre::run(
            &song,
            &a.stem,
            &a.repro,
            pair(&a.bars),
            a.part.as_deref(),
            a.midi.as_deref(),
        )?,
        Cmd::Resynth(a) => resynth::run(&song, &a.stem, a.fft, &a.out)?,
    };
    print(&value)
}
