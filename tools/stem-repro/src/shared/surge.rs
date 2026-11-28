use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clack_extensions::params::{ParamInfoBuffer, PluginParams};
use clack_extensions::state::PluginState;
use clack_host::events::Match;
use clack_host::events::event_types::{NoteOffEvent, NoteOnEvent, ParamValueEvent};
use clack_host::prelude::*;
use clack_host::process::StartedPluginAudioProcessor;
use clack_host::process::audio_buffers::{
    AudioPortBuffer, AudioPortBufferType, AudioPorts, InputChannel,
};
use clack_host::utils::ClapId;
use num_traits::ToPrimitive;
use sha2::{Digest, Sha256};

use crate::shared::notes::Event;
use crate::shared::signal::{Frame, RATE};

const VERSION: &str = "1.3.4";
const DEB_SHA256: &str = "a6e55064487f624147d515b9ae5fc79a568b69746675b2083abde628ca7bb151";
const PLUGIN: &str = "usr/lib/clap/Surge XT.clap";
const DATA: &str = "usr/share/surge-xt";
const BLOCK: usize = 512;

pub struct Install {
    pub clap: PathBuf,
    pub patches: PathBuf,
}

fn cache_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(dir).join("stem-repro"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".cache").join("stem-repro"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn unpack(deb: &[u8], into: &Path) -> Result<()> {
    let mut archive = ar::Archive::new(Cursor::new(deb));
    while let Some(entry) = archive.next_entry() {
        let mut entry = entry?;
        if entry.header().identifier() != b"data.tar.xz" {
            continue;
        }
        let mut compressed = Vec::new();
        entry.read_to_end(&mut compressed)?;
        let mut tar = tar::Archive::new(liblzma::read::XzDecoder::new(compressed.as_slice()));
        for file in tar.entries()? {
            let mut file = file?;
            let path = file.path()?.into_owned();
            let relative = path.strip_prefix("./").unwrap_or(&path);
            if relative.starts_with(PLUGIN) || relative.starts_with(DATA) {
                file.unpack_in(into)?;
            }
        }
        return Ok(());
    }
    bail!("the Surge XT package has no data.tar.xz")
}

pub fn install() -> Result<Install> {
    if let Some(clap) = std::env::var_os("SURGE_CLAP") {
        let patches = std::env::var_os("SURGE_PATCHES")
            .map_or_else(|| PathBuf::from("/usr/share/surge-xt"), PathBuf::from);
        return Ok(Install {
            clap: clap.into(),
            patches,
        });
    }
    let root = cache_root()?.join(format!("surge-xt-{VERSION}"));
    if !root.join(PLUGIN).exists() {
        let url = format!(
            "https://github.com/surge-synthesizer/releases-xt/releases/download/{VERSION}/surge-xt-linux-x64-{VERSION}.deb"
        );
        eprintln!("surge: downloading {url}");
        let deb = reqwest::blocking::get(&url)?.error_for_status()?.bytes()?;
        let digest = hex(&Sha256::digest(&deb));
        if digest != DEB_SHA256 {
            bail!("{url}: sha256 {digest}, expected {DEB_SHA256}");
        }
        let staging = root.with_extension("partial");
        if staging.exists() {
            std::fs::remove_dir_all(&staging)?;
        }
        std::fs::create_dir_all(&staging)?;
        unpack(&deb, &staging)?;
        std::fs::rename(&staging, &root)?;
    }
    Ok(Install {
        clap: root.join(PLUGIN),
        patches: root.join(DATA),
    })
}

pub fn patches(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = std::fs::read_dir(root)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("patches_"))
        })
        .collect();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "fxp") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

struct Shared;

impl SharedHandler<'_> for Shared {
    fn request_restart(&self) {}
    fn request_process(&self) {}
    fn request_callback(&self) {}
}

struct Host;

impl HostHandlers for Host {
    type Shared<'a> = Shared;
    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();
}

#[must_use]
pub fn parameter_name(name: &str) -> String {
    let mapped: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let mut out = String::with_capacity(mapped.len());
    for c in mapped.chars() {
        if !(c == '_' && out.ends_with('_')) {
            out.push(c);
        }
    }
    out.trim_matches('_').to_string()
}

pub struct Surge {
    params: PluginParams,
    state: PluginState,
    ids: HashMap<String, ClapId>,
    pending: Vec<(ClapId, f64)>,
    processor: Option<StartedPluginAudioProcessor<Host>>,
    instance: PluginInstance<Host>,
    ports: (AudioPorts, AudioPorts),
}

#[expect(unsafe_code, reason = "loading a CLAP plugin runs its native code")]
fn entry(path: &Path) -> Result<PluginEntry> {
    unsafe { PluginEntry::load(path) }.with_context(|| path.display().to_string())
}

impl Surge {
    pub fn new(install: &Install) -> Result<Self> {
        let entry = entry(&install.clap)?;
        let info = HostInfo::new(
            "stem-repro",
            "ai-produced",
            "https://example.invalid",
            "0.0.0",
        )?;
        let mut instance = PluginInstance::<Host>::new(
            |()| Shared,
            |_| (),
            &entry,
            c"org.surge-synth-team.surge-xt",
            &info,
        )?;
        let shared = instance.plugin_shared_handle();
        let params: PluginParams = shared
            .get_extension()
            .ok_or_else(|| anyhow!("Surge XT has no params extension"))?;
        let state: PluginState = shared
            .get_extension()
            .ok_or_else(|| anyhow!("Surge XT has no state extension"))?;
        let handle = instance.plugin_handle();
        let mut buffer = ParamInfoBuffer::new();
        let mut ids = HashMap::new();
        for i in 0..params.count(&handle) {
            if let Some(info) = params.get_info(&handle, i, &mut buffer) {
                ids.entry(parameter_name(&String::from_utf8_lossy(info.name)))
                    .or_insert(info.id);
            }
        }
        let configuration = PluginAudioConfiguration {
            sample_rate: RATE,
            min_frames_count: 1,
            max_frames_count: u32::try_from(BLOCK)?,
        };
        let stopped = instance.activate(|_, ()| (), configuration)?;
        let processor = stopped
            .start_processing()
            .map_err(|_| anyhow!("Surge XT did not start processing"))?;
        Ok(Self {
            params,
            state,
            ids,
            pending: Vec::new(),
            processor: Some(processor),
            instance,
            ports: (
                AudioPorts::with_capacity(2, 1),
                AudioPorts::with_capacity(2, 1),
            ),
        })
    }

    fn id(&self, name: &str) -> Result<ClapId> {
        self.ids
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("Surge XT has no parameter {name}"))
    }

    pub fn set(&mut self, name: &str, value: f64) -> Result<()> {
        let id = self.id(name)?;
        self.pending.retain(|(p, _)| *p != id);
        self.pending.push((id, value));
        Ok(())
    }

    pub fn value(&mut self, name: &str) -> Result<f64> {
        let id = self.id(name)?;
        if let Some((_, v)) = self.pending.iter().find(|(p, _)| *p == id) {
            return Ok(*v);
        }
        let handle = self.instance.plugin_handle();
        self.params
            .get_value(&handle, id)
            .ok_or_else(|| anyhow!("Surge XT did not report {name}"))
    }

    pub fn load(&mut self, fxp: &Path, params: &[(String, f64)]) -> Result<()> {
        let data = std::fs::read(fxp).with_context(|| fxp.display().to_string())?;
        if data.len() <= 60 {
            bail!("{}: not a Surge patch", fxp.display());
        }
        let handle = self.instance.plugin_handle();
        self.state
            .load(&handle, &mut &data[60..])
            .map_err(|_| anyhow!("{}: Surge XT refused the patch", fxp.display()))?;
        self.pending.clear();
        self.run(&[], 0.3)?;
        for (name, value) in params {
            self.set(name, *value)?;
        }
        Ok(())
    }

    fn run(&mut self, events: &[Event], duration: f64) -> Result<Vec<Frame>> {
        let n = (duration * RATE).to_usize().context("render length")?;
        let starts: Vec<usize> = events
            .iter()
            .map(|e| (e.time * RATE).max(0.0).to_usize().context("event time"))
            .collect::<Result<_>>()?;
        let processor = self
            .processor
            .as_mut()
            .ok_or_else(|| anyhow!("Surge XT is not processing"))?;
        let mut out = Vec::with_capacity(n);
        let mut inputs = [vec![0f32; BLOCK], vec![0f32; BLOCK]];
        let mut outputs = [vec![0f32; BLOCK], vec![0f32; BLOCK]];
        let mut next = 0;
        let mut start = 0;
        while start < n {
            let len = BLOCK.min(n - start);
            let mut buffer = EventBuffer::new();
            for (id, value) in self.pending.drain(..) {
                buffer.push(&ParamValueEvent::new(
                    0,
                    id,
                    Pckn::new(Match::All, Match::All, Match::All, Match::All),
                    value,
                ));
            }
            while next < events.len() && starts[next] < start + len {
                let event = events[next];
                let at = u32::try_from(starts[next].saturating_sub(start))?;
                let pckn = Pckn::new(0u16, 0u16, u16::from(event.pitch), Match::All);
                match event.velocity {
                    Some(v) => buffer.push(&NoteOnEvent::new(at, pckn, f64::from(v) / 127.0)),
                    None => buffer.push(&NoteOffEvent::new(at, pckn, 64.0 / 127.0)),
                }
                next += 1;
            }
            let input_events = InputEvents::from_buffer(&buffer);
            let mut sink = EventBuffer::new();
            let mut output_events = OutputEvents::from_buffer(&mut sink);
            let input_audio = self.ports.0.with_input_buffers([AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_input_only(
                    inputs
                        .iter_mut()
                        .map(|b| InputChannel::constant(&mut b[..len])),
                ),
            }]);
            let mut output_audio = self.ports.1.with_output_buffers([AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_output_only(
                    outputs.iter_mut().map(|b| &mut b[..len]),
                ),
            }]);
            processor
                .process(
                    &input_audio,
                    &mut output_audio,
                    &input_events,
                    &mut output_events,
                    None,
                    None,
                )
                .map_err(|e| anyhow!("Surge XT failed to process: {e}"))?;
            out.extend((0..len).map(|i| [f64::from(outputs[0][i]), f64::from(outputs[1][i])]));
            start += len;
        }
        Ok(out)
    }

    pub fn render(&mut self, events: &[Event], duration: f64) -> Result<Vec<Frame>> {
        let out = self.run(events, duration)?;
        if let Some(processor) = self.processor.as_mut() {
            processor.reset();
        }
        Ok(out)
    }

    pub fn render_mono(
        &mut self,
        events: &[Event],
        duration: f64,
        n: usize,
    ) -> Result<Option<Vec<f64>>> {
        let mut m = crate::shared::signal::mono(&self.render(events, duration)?);
        m.truncate(n);
        let silent = m.iter().fold(0.0f64, |a, v| a.max(v.abs())) < 1e-5;
        Ok((!silent && m.iter().all(|v| v.is_finite())).then_some(m))
    }
}

impl Drop for Surge {
    fn drop(&mut self) {
        if let Some(processor) = self.processor.take() {
            self.instance.deactivate(processor.stop_processing());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::signal::SR;

    #[test]
    #[ignore = "downloads Surge XT"]
    fn factory_patches_render_sound() {
        let install = install().unwrap();
        let all = patches(&install.patches).unwrap();
        assert!(all.len() > 1000, "{}", all.len());
        let mut surge = Surge::new(&install).unwrap();
        let note = [
            Event {
                time: 0.0,
                pitch: 60,
                velocity: Some(100),
            },
            Event {
                time: 0.5,
                pitch: 60,
                velocity: None,
            },
        ];
        for name in ["Init Saw", "Init Wavetable"] {
            let patch = all
                .iter()
                .find(|p| p.file_stem().is_some_and(|s| s == name))
                .unwrap();
            surge.load(patch, &[]).unwrap();
            let out = surge.render(&note, 1.0).unwrap();
            assert_eq!(out.len(), usize::try_from(SR).unwrap());
            let level = crate::shared::signal::peak(&out);
            assert!(level > 0.01, "{name}: peak {level}");
        }
    }

    #[test]
    fn parameter_names_are_snake_case() {
        assert_eq!(parameter_name("A Amp EG Attack"), "a_amp_eg_attack");
        assert_eq!(parameter_name("M1: -"), "m1");
        assert_eq!(
            parameter_name("A Filter 1 FEG Mod Amount"),
            "a_filter_1_feg_mod_amount"
        );
    }
}
