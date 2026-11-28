use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use num_traits::ToPrimitive;
use uuid::Uuid;

#[derive(Parser)]
#[command(
    about = "Build a REAPER project from a multitrack MIDI file: tempo, section markers from MIDI marker events, one track per MIDI track with the notes embedded, and an audio track for the rendered vocal. No plugins are set."
)]
struct Args {
    midi: PathBuf,
    #[arg(long, help = "project .rpp")]
    out: PathBuf,
    #[arg(
        long,
        value_name = "TRACK=WAV",
        help = "audio track in place of the MIDI track of the same name; WAV is relative to the project"
    )]
    audio: Vec<String>,
    #[arg(long, default_value_t = 48000)]
    sample_rate: u32,
}

enum Node {
    Line(Vec<String>),
    Element(Element),
}

struct Element {
    tag: String,
    attributes: Vec<String>,
    children: Vec<Node>,
}

impl Element {
    fn new(tag: &str, attributes: &[&str], children: Vec<Node>) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: attributes.iter().map(|a| (*a).to_string()).collect(),
            children,
        }
    }

    fn write(&self, out: &mut String, depth: usize) {
        let indent = "  ".repeat(depth);
        let head = std::iter::once(self.tag.clone())
            .chain(self.attributes.iter().map(|a| quote(a)))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(out, "{indent}<{head}");
        for child in &self.children {
            match child {
                Node::Line(values) => {
                    let line = values
                        .iter()
                        .map(|v| quote(v))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = writeln!(out, "{indent}  {line}");
                }
                Node::Element(element) => element.write(out, depth + 1),
            }
        }
        let _ = writeln!(out, "{indent}>");
    }
}

fn quote(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let needs = value.chars().any(char::is_whitespace) || value.starts_with(['"', '\'', '`']);
    if !needs {
        return value.to_string();
    }
    let mark = ['"', '\'', '`']
        .into_iter()
        .find(|mark| !value.contains(*mark))
        .unwrap_or('"');
    format!("{mark}{value}{mark}")
}

fn line(values: &[&str]) -> Node {
    Node::Line(values.iter().map(|v| (*v).to_string()).collect())
}

fn guid(seed: &str) -> String {
    format!(
        "{{{}}}",
        Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes())
            .to_string()
            .to_uppercase()
    )
}

fn general(value: f64) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

struct Conductor {
    tempo: u32,
    numerator: u8,
    denominator: u8,
    markers: Vec<(u64, String)>,
}

fn conductor(track: &[TrackEvent]) -> Conductor {
    let mut found = Conductor {
        tempo: 500_000,
        numerator: 4,
        denominator: 4,
        markers: Vec::new(),
    };
    let mut tick = 0u64;
    for event in track {
        tick += u64::from(u32::from(event.delta));
        match event.kind {
            TrackEventKind::Meta(MetaMessage::Tempo(tempo)) if tick == 0 => {
                found.tempo = u32::from(tempo);
            }
            TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, power, _, _))
                if tick == 0 =>
            {
                found.numerator = numerator;
                found.denominator = 1 << power;
            }
            TrackEventKind::Meta(MetaMessage::Marker(text)) => {
                found
                    .markers
                    .push((tick, String::from_utf8_lossy(text).into_owned()));
            }
            _ => {}
        }
    }
    found
}

fn message_bytes(channel: u8, message: MidiMessage) -> Vec<u8> {
    match message {
        MidiMessage::NoteOff { key, vel } => vec![0x80 | channel, key.into(), vel.into()],
        MidiMessage::NoteOn { key, vel } => vec![0x90 | channel, key.into(), vel.into()],
        MidiMessage::Aftertouch { key, vel } => vec![0xa0 | channel, key.into(), vel.into()],
        MidiMessage::Controller { controller, value } => {
            vec![0xb0 | channel, controller.into(), value.into()]
        }
        MidiMessage::ProgramChange { program } => vec![0xc0 | channel, program.into()],
        MidiMessage::ChannelAftertouch { vel } => vec![0xd0 | channel, vel.into()],
        MidiMessage::PitchBend { bend } => {
            let value = bend.0.as_int();
            let [low, _] = (value & 0x7f).to_le_bytes();
            let [high, _] = (value >> 7).to_le_bytes();
            vec![0xe0 | channel, low, high]
        }
    }
}

fn midi_events(track: &[TrackEvent], length_ticks: u64) -> Vec<Node> {
    let mut events = Vec::new();
    let mut last = 0u64;
    let mut tick = 0u64;
    for event in track {
        tick += u64::from(u32::from(event.delta));
        let bytes = match &event.kind {
            TrackEventKind::Midi { channel, message } => message_bytes(channel.as_int(), *message),
            TrackEventKind::Escape(_) | TrackEventKind::SysEx(_) | TrackEventKind::Meta(_) => {
                continue;
            }
        };
        let mut values = vec!["E".to_string(), (tick - last).to_string()];
        values.extend(bytes.iter().map(|byte| format!("{byte:02x}")));
        events.push(Node::Line(values));
        last = tick;
    }
    events.push(line(&[
        "E",
        &length_ticks.saturating_sub(last).to_string(),
        "b0",
        "7b",
        "00",
    ]));
    events
}

fn track_name(track: &[TrackEvent]) -> String {
    track
        .iter()
        .find_map(|event| match event.kind {
            TrackEventKind::Meta(MetaMessage::TrackName(name)) => {
                Some(String::from_utf8_lossy(name).into_owned())
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn track_element(name: &str, item_name: &str, source: Element, length_seconds: f64) -> Element {
    let length = format!("{length_seconds:.6}");
    let item = Element::new(
        "ITEM",
        &[],
        vec![
            line(&["POSITION", "0"]),
            line(&["LENGTH", &length]),
            line(&["NAME", item_name]),
            line(&["GUID", &guid(&format!("item/{name}"))]),
            Node::Element(source),
        ],
    );
    let id = guid(&format!("track/{name}"));
    Element::new(
        "TRACK",
        &[&id],
        vec![
            line(&["NAME", name]),
            line(&["TRACKID", &id]),
            Node::Element(item),
        ],
    )
}

fn audio_track(name: &str, path: &str, length_seconds: f64) -> Element {
    let source = Element::new("SOURCE", &["WAVE"], vec![line(&["FILE", path])]);
    let item_name = Path::new(path)
        .file_name()
        .map_or_default(|n| n.to_string_lossy().into_owned());
    track_element(name, &item_name, source, length_seconds)
}

pub fn run() -> Result<()> {
    execute(&Args::parse())
}

fn execute(args: &Args) -> Result<()> {
    let data = fs::read(&args.midi).with_context(|| args.midi.display().to_string())?;
    let smf = Smf::parse(&data)?;
    let Timing::Metrical(ppq) = smf.header.timing else {
        return Err(anyhow!("timecode MIDI files are not supported"));
    };
    let ppq = u64::from(ppq.as_int());
    let first = smf
        .tracks
        .first()
        .ok_or_else(|| anyhow!("MIDI file has no tracks"))?;
    let found = conductor(first);
    let bpm = 60_000_000.0 / f64::from(found.tempo);
    let bar_ticks = ppq * u64::from(found.numerator) * 4 / u64::from(found.denominator);
    let end_ticks = smf
        .tracks
        .iter()
        .map(|track| {
            track
                .iter()
                .map(|e| u64::from(u32::from(e.delta)))
                .sum::<u64>()
        })
        .max()
        .unwrap_or(0);
    let length_ticks = end_ticks.div_ceil(bar_ticks) * bar_ticks;
    let seconds = |ticks: u64| -> Result<f64> {
        let ticks = ticks.to_f64().context("ticks")?;
        Ok(ticks / ppq.to_f64().context("ppq")? * 60.0 / bpm)
    };
    let length_seconds = seconds(length_ticks)?;
    let mut audio: Vec<(String, String)> = args
        .audio
        .iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(track, wav)| (track.to_string(), wav.to_string()))
                .ok_or_else(|| anyhow!("--audio needs TRACK=WAV, got {entry}"))
        })
        .collect::<Result<_>>()?;
    let mut children = vec![
        line(&[
            "TEMPO",
            &general(bpm),
            &found.numerator.to_string(),
            &found.denominator.to_string(),
        ]),
        line(&["SAMPLERATE", &args.sample_rate.to_string(), "0", "0"]),
    ];
    for (index, (tick, name)) in found.markers.iter().enumerate() {
        let index = (index + 1).to_string();
        let at = format!("{:.6}", seconds(*tick)?);
        let id = guid(&format!("marker/{index}"));
        children.push(line(&[
            "MARKER", &index, &at, name, "0", "0", "1", "B", &id,
        ]));
    }
    let mut tracks = 0;
    for track in &smf.tracks[1..] {
        let name = track_name(track);
        let element = if let Some(at) = audio.iter().position(|(track, _)| *track == name) {
            let (_, path) = audio.remove(at);
            audio_track(&name, &path, length_seconds)
        } else {
            let mut source_lines = vec![line(&["HASDATA", "1", &ppq.to_string(), "QN"])];
            source_lines.extend(midi_events(track, length_ticks));
            let source = Element::new("SOURCE", &["MIDI"], source_lines);
            track_element(&name, &name, source, length_seconds)
        };
        children.push(Node::Element(element));
        tracks += 1;
    }
    for (name, path) in &audio {
        children.push(Node::Element(audio_track(name, path, length_seconds)));
        tracks += 1;
    }
    let project = Element::new("REAPER_PROJECT", &["0.1", "7.0", "0"], children);
    let mut text = String::new();
    project.write(&mut text, 0);
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, text)?;
    println!(
        "{}: {} BPM, {} markers, {tracks} tracks, {length_seconds:.2} s",
        args.out.display(),
        general(bpm),
        found.markers.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::test_repository::song;

    #[test]
    fn reproduces_the_reaper_project() {
        let out = tempfile::tempdir().expect("tempdir");
        let rpp = out.path().join("mou-ikkai.rpp");
        let args = Args::parse_from([
            "midi-rpp".as_ref(),
            song().join("02-composition/generated/full.mid").as_os_str(),
            "--out".as_ref(),
            rpp.as_os_str(),
            "--audio".as_ref(),
            "Vocal=../03-vocal/render/mou-ikkai_Vocal.wav".as_ref(),
        ]);
        execute(&args).expect("midi-rpp");
        let committed = song().join("04-reaper/mou-ikkai.rpp");
        assert_eq!(
            fs::read(rpp).expect("rpp"),
            fs::read(committed).expect("committed")
        );
    }
}
