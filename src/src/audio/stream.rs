use super::dsp::VoiceDSP;
use crate::soundboard::player::SoundboardPlayer;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, Device, SampleFormat, Stream, StreamConfig, SupportedBufferSize,
    SupportedStreamConfigRange,
};
use log::{error, info, warn};
use parking_lot::Mutex;
use rtrb::RingBuffer;
use rubato::{
    audioadapter_buffers::direct::InterleavedSlice,
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct SafeStream(Stream);
unsafe impl Send for SafeStream {}
unsafe impl Sync for SafeStream {}

impl std::ops::Deref for SafeStream {
    type Target = Stream;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevicesState {
    pub inputs: Vec<AudioDeviceInfo>,
    pub outputs: Vec<AudioDeviceInfo>,
    pub current_input: Option<usize>,
    pub current_monitor: Option<usize>,
    pub block_size: usize,
    pub mic_gain: f32,
    pub monitor_gain: f32,
    pub hear_myself: bool,
    pub hear_soundboard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Meters {
    pub in_peak: f32,
    pub in_rms: f32,
    pub out_peak: f32,
    pub out_rms: f32,
    pub is_muted: bool,
    pub hear_myself: bool,
    pub engine_active: bool,
}

/// Sentinel stored in `selected_monitor_name` when the monitor output is
/// explicitly disabled (mirrors Python `"none"`).
const MONITOR_DISABLED_SENTINEL: &str = "none";

/// Number of stereo blocks buffered in the cross-thread rings.
/// Mirrors Python `queue.Queue(maxsize=32)`.
const RING_BLOCKS: usize = 32;

/// Virtual-sink discovery retries (PipeWire may need a moment after
/// `module-null-sink` is loaded). Mirrors Python's brief wait + retry.
const VIRTUAL_SINK_RETRIES: u32 = 10;
const VIRTUAL_SINK_RETRY_DELAY_MS: u64 = 100;

pub struct AudioStreamEngine {
    pub sample_rate: u32,
    /// Sample rate actually negotiated with the input device.
    /// The whole engine (DSP, soundboard, virtual/monitor outputs, pacat)
    /// runs on this single clock so producer and consumer rates can never
    /// drift apart on any hardware (8 kHz BT headsets, 44.1 kHz USB mics,
    /// 48/96 kHz cards, PipeWire/Pulse/ALSA alike).
    pub effective_rate: AtomicU32,
    pub block_size: AtomicUsize,
    pub dsp: Arc<Mutex<VoiceDSP>>,
    pub soundboard: Arc<SoundboardPlayer>,

    pub is_running: Arc<AtomicBool>,
    pub is_muted: Arc<AtomicBool>,
    pub hear_myself: Arc<AtomicBool>,
    pub hear_soundboard: Arc<AtomicBool>,

    pub mic_gain_bits: Arc<AtomicU32>,
    pub monitor_gain_bits: Arc<AtomicU32>,

    pub in_peak_bits: Arc<AtomicU32>,
    pub in_rms_bits: Arc<AtomicU32>,
    pub out_peak_bits: Arc<AtomicU32>,
    pub out_rms_bits: Arc<AtomicU32>,

    pub selected_input_name: Mutex<Option<String>>,
    pub selected_monitor_name: Mutex<Option<String>>,

    input_stream: Mutex<Option<SafeStream>>,
    virtual_stream: Mutex<Option<VirtualOutputStream>>,
    monitor_stream: Mutex<Option<SafeStream>>,
}

fn get_device_name(dev: &Device) -> String {
    dev.description()
        .ok()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|| dev.to_string())
}

/// Returns true for Audiover's own virtual devices, ALSA null/dummy devices,
/// and black-hole pseudo-devices that discard audio or produce silence.
pub(crate) fn is_virtual_device_name(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    lower.contains("audiover")
        || lower.contains("virtual_sink")
        || lower.contains("null_sink")
        || lower.contains("remap_source")
        || lower.contains("discard all samples")
        || lower.contains("generate zero samples")
        || lower == "null"
        || lower.contains("dummy")
}

/// Checks both device description/driver and name to filter out unusable devices.
pub(crate) fn is_unusable_or_virtual_device(dev: &Device) -> bool {
    if let Ok(desc) = dev.description() {
        if let Some(driver) = desc.driver() {
            let drv = driver.trim().to_lowercase();
            if drv == "null" || drv == "dummy" {
                return true;
            }
        }
        if is_virtual_device_name(desc.name()) {
            return true;
        }
    }
    let name = get_device_name(dev);
    is_virtual_device_name(&name)
}

/// Case-insensitive substring match, skipping virtual and unusable devices.
fn match_device_by_name(devices: Vec<Device>, saved: &str) -> Option<Device> {
    let clean = saved.trim().to_lowercase();
    if clean.is_empty() || is_virtual_device_name(&clean) {
        return None;
    }
    devices.into_iter().find(|d| {
        let name = get_device_name(d);
        let lower = name.to_lowercase();
        (lower == clean || lower.contains(&clean as &str)) && !is_unusable_or_virtual_device(d)
    })
}

fn i16_to_f32(s: i16) -> f32 {
    (s as f32) / 32768.0
}

fn u16_to_f32(s: u16) -> f32 {
    (s as f32 - 32768.0) / 32768.0
}

fn f32_to_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0) as i16
}

fn f32_to_u16(v: f32) -> u16 {
    (v.clamp(-1.0, 1.0) * 32768.0 + 32768.0).clamp(0.0, 65535.0) as u16
}

/// Real-time streaming stereo resampler powered by rubato band-limited sinc interpolation.
/// Designed for zero-allocation processing on audio block callbacks.
pub struct StreamingStereoResampler {
    resampler: Async<f32>,
    in_buf: Vec<f32>,
    out_buf: Vec<f32>,
    chunk_size: usize,
}

impl StreamingStereoResampler {
    pub fn new(in_sr: u32, out_sr: u32, chunk_size: usize) -> Option<Self> {
        if in_sr == out_sr {
            return None;
        }
        let in_sr_f = in_sr.max(4000) as f64;
        let out_sr_f = out_sr.max(4000) as f64;
        let ratio = out_sr_f / in_sr_f;

        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: None,
            oversampling_factor: 128,
            interpolation: SincInterpolationType::Cubic,
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler = Async::<f32>::new_sinc(
            ratio,
            1.1,
            &params,
            chunk_size,
            2,
            FixedAsync::Input,
        )
        .map_err(|e| {
            warn!("Failed to initialize streaming resampler: {}", e);
            e
        })
        .ok()?;

        let max_out = resampler.output_frames_max();
        Some(Self {
            resampler,
            in_buf: vec![0.0f32; chunk_size * 2],
            out_buf: vec![0.0f32; (max_out + 64) * 2],
            chunk_size,
        })
    }

    pub fn resample_and_push(&mut self, frames: &[[f32; 2]], prod: &mut rtrb::Producer<[f32; 2]>) {
        let n = frames.len();
        if n == 0 {
            return;
        }

        if self.in_buf.len() < self.chunk_size * 2 {
            self.in_buf.resize(self.chunk_size * 2, 0.0);
        }
        for (i, f) in frames.iter().enumerate().take(self.chunk_size) {
            self.in_buf[i * 2] = f[0];
            self.in_buf[i * 2 + 1] = f[1];
        }
        for i in n..self.chunk_size {
            self.in_buf[i * 2] = 0.0;
            self.in_buf[i * 2 + 1] = 0.0;
        }

        let in_adapter = match InterleavedSlice::new(&self.in_buf, 2, self.chunk_size) {
            Ok(a) => a,
            Err(_) => {
                for f in frames {
                    let _ = prod.push(*f);
                }
                return;
            }
        };

        let max_out = self.resampler.output_frames_max();
        if self.out_buf.len() < max_out * 2 {
            self.out_buf.resize(max_out * 2, 0.0);
        }

        let mut out_adapter = match InterleavedSlice::new_mut(&mut self.out_buf, 2, max_out) {
            Ok(a) => a,
            Err(_) => {
                for f in frames {
                    let _ = prod.push(*f);
                }
                return;
            }
        };

        let indexing = if n < self.chunk_size {
            Indexing::new().partial_len(n)
        } else {
            Indexing::new()
        };

        match self.resampler.process_into_buffer(&in_adapter, &mut out_adapter, Some(&indexing)) {
            Ok((_nbr_in, nbr_out)) => {
                for i in 0..nbr_out {
                    let frame = [self.out_buf[i * 2], self.out_buf[i * 2 + 1]];
                    let _ = prod.push(frame);
                }
            }
            Err(_) => {
                for f in frames {
                    let _ = prod.push(*f);
                }
            }
        }
    }
}

fn make_output_callback_f32(
    mut cons: rtrb::Consumer<[f32; 2]>,
    capacity: usize,
    block_size: usize,
) -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut last_sample = [0.0f32, 0.0];
    let max_headroom = capacity.saturating_sub(block_size * 2);

    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let frames = data.len() / 2;
        if cons.slots() > max_headroom {
            let _ = cons.pop();
        }
        for i in 0..frames {
            match cons.pop() {
                Ok(f) => {
                    last_sample = f;
                    data[i * 2] = f[0];
                    data[i * 2 + 1] = f[1];
                }
                Err(_) => {
                    last_sample[0] *= 0.85;
                    last_sample[1] *= 0.85;
                    data[i * 2] = last_sample[0];
                    data[i * 2 + 1] = last_sample[1];
                }
            }
        }
    }
}

fn make_output_callback_i16(
    mut cons: rtrb::Consumer<[f32; 2]>,
    capacity: usize,
    block_size: usize,
) -> impl FnMut(&mut [i16], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut last_sample = [0.0f32, 0.0];
    let max_headroom = capacity.saturating_sub(block_size * 2);

    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
        let frames = data.len() / 2;
        if cons.slots() > max_headroom {
            let _ = cons.pop();
        }
        for i in 0..frames {
            match cons.pop() {
                Ok(f) => {
                    last_sample = f;
                    data[i * 2] = f32_to_i16(f[0]);
                    data[i * 2 + 1] = f32_to_i16(f[1]);
                }
                Err(_) => {
                    last_sample[0] *= 0.85;
                    last_sample[1] *= 0.85;
                    data[i * 2] = f32_to_i16(last_sample[0]);
                    data[i * 2 + 1] = f32_to_i16(last_sample[1]);
                }
            }
        }
    }
}

fn make_output_callback_u16(
    mut cons: rtrb::Consumer<[f32; 2]>,
    capacity: usize,
    block_size: usize,
) -> impl FnMut(&mut [u16], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut last_sample = [0.0f32, 0.0];
    let max_headroom = capacity.saturating_sub(block_size * 2);

    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
        let frames = data.len() / 2;
        if cons.slots() > max_headroom {
            let _ = cons.pop();
        }
        for i in 0..frames {
            match cons.pop() {
                Ok(f) => {
                    last_sample = f;
                    data[i * 2] = f32_to_u16(f[0]);
                    data[i * 2 + 1] = f32_to_u16(f[1]);
                }
                Err(_) => {
                    last_sample[0] *= 0.85;
                    last_sample[1] *= 0.85;
                    data[i * 2] = f32_to_u16(last_sample[0]);
                    data[i * 2 + 1] = f32_to_u16(last_sample[1]);
                }
            }
        }
    }
}

/// Owns the per-block DSP + mixing state for the input callback thread.
///
/// The soundboard is mixed here exactly once per input block (Python parity:
/// `get_mix_block` is called once in `_input_callback`). Mixing in the output
/// callbacks instead would advance track positions twice per period and play
/// sounds at double speed with tearing between monitor and virtual outputs.
struct InputProcessor {
    dsp: Arc<Mutex<VoiceDSP>>,
    soundboard: Arc<SoundboardPlayer>,
    mic_gain_bits: Arc<AtomicU32>,
    monitor_gain_bits: Arc<AtomicU32>,
    is_muted: Arc<AtomicBool>,
    hear_myself: Arc<AtomicBool>,
    hear_soundboard: Arc<AtomicBool>,
    in_peak_bits: Arc<AtomicU32>,
    in_rms_bits: Arc<AtomicU32>,
    out_peak_bits: Arc<AtomicU32>,
    out_rms_bits: Arc<AtomicU32>,
    virt_prod: rtrb::Producer<[f32; 2]>,
    mon_prod: rtrb::Producer<[f32; 2]>,
    virt_resampler: Option<StreamingStereoResampler>,
    mon_resampler: Option<StreamingStereoResampler>,
    // Scratch buffers sized to the engine block size.
    mono: Vec<f32>,
    dsp_out: Vec<f32>,
    voice_stereo: Vec<[f32; 2]>,
    sb_block: Vec<[f32; 2]>,
    mon_scratch: Vec<[f32; 2]>,
}

impl InputProcessor {
    fn handle_mono_block(&mut self, raw: &[f32]) {
        let n = raw.len();
        if n == 0 {
            return;
        }
        if self.mono.len() < n {
            self.mono.resize(n, 0.0);
            self.dsp_out.resize(n, 0.0);
            self.voice_stereo.resize(n, [0.0, 0.0]);
            self.sb_block.resize(n, [0.0, 0.0]);
            self.mon_scratch.resize(n, [0.0, 0.0]);
        }

        // 1. Input meters on the RAW microphone signal (pre-mute / pre-gain),
        //    mirroring Python which meters `raw_mono` before mute/gain.
        let mut peak = 0.0f32;
        let mut sum_sq = 0.0f32;
        for &s in raw {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            sum_sq += s * s;
        }
        self.in_peak_bits.store(peak.to_bits(), Ordering::Relaxed);
        let rms = (sum_sq / (n as f32).max(1.0)).sqrt();
        self.in_rms_bits.store(rms.to_bits(), Ordering::Relaxed);

        // 2. Mute + mic gain.
        let is_muted = self.is_muted.load(Ordering::Relaxed);
        let mic_gain = f32::from_bits(self.mic_gain_bits.load(Ordering::Relaxed));
        for (i, &s) in raw.iter().enumerate() {
            self.mono[i] = if is_muted { 0.0 } else { s * mic_gain };
        }

        // 3. DSP pipeline (mono).
        self.dsp.lock().process(&self.mono[..n], &mut self.dsp_out[..n]);
        for (i, &s) in self.dsp_out.iter().enumerate().take(n) {
            self.voice_stereo[i] = [s, s];
        }

        // 4. Soundboard block, mixed ONCE per input block.
        for f in self.sb_block.iter_mut().take(n) {
            *f = [0.0, 0.0];
        }
        self.soundboard.mix_into(&mut self.sb_block[..n], 1.0);

        // 5. Virtual-mic mix: voice + soundboard with soft limiter.
        //    Mirrors Python `virtual_mic_mix = tanh(...)`.
        let mut out_peak = 0.0f32;
        let mut out_sum_sq = 0.0f32;
        for i in 0..n {
            let l = (self.voice_stereo[i][0] + self.sb_block[i][0]).tanh();
            let r = (self.voice_stereo[i][1] + self.sb_block[i][1]).tanh();
            self.voice_stereo[i] = [l, r];
            let a = l.abs().max(r.abs());
            if a > out_peak {
                out_peak = a;
            }
            out_sum_sq += l * l + r * r;
        }
        self.out_peak_bits.store(out_peak.to_bits(), Ordering::Relaxed);
        let out_rms = (out_sum_sq / ((n * 2) as f32).max(1.0)).sqrt();
        self.out_rms_bits.store(out_rms.to_bits(), Ordering::Relaxed);

        if let Some(ref mut vr) = self.virt_resampler {
            vr.resample_and_push(&self.voice_stereo[..n], &mut self.virt_prod);
        } else {
            for i in 0..n {
                let _ = self.virt_prod.push(self.voice_stereo[i]);
            }
        }

        // 6. Monitor (headphone) mix: voice only if hear_myself, soundboard
        //    only if hear_soundboard, with soft limiter on gained signal.
        //    Mirrors Python `mon_mix = tanh(mon_mix * monitor_gain)`.
        let hear_voice = self.hear_myself.load(Ordering::Relaxed);
        let hear_sb = self.hear_soundboard.load(Ordering::Relaxed);
        let mon_gain = f32::from_bits(self.monitor_gain_bits.load(Ordering::Relaxed));
        for i in 0..n {
            let v = if hear_voice { self.dsp_out[i] } else { 0.0 };
            let s = if hear_sb { self.sb_block[i] } else { [0.0, 0.0] };
            let frame = [(v + s[0]) * mon_gain, (v + s[1]) * mon_gain];
            self.mon_scratch[i] = [frame[0].tanh(), frame[1].tanh()];
        }

        if let Some(ref mut mr) = self.mon_resampler {
            mr.resample_and_push(&self.mon_scratch[..n], &mut self.mon_prod);
        } else {
            for i in 0..n {
                let _ = self.mon_prod.push(self.mon_scratch[i]);
            }
        }
    }
}

impl AudioStreamEngine {
    pub fn new(
        sample_rate: u32,
        block_size: usize,
        dsp: Arc<Mutex<VoiceDSP>>,
        soundboard: Arc<SoundboardPlayer>,
    ) -> Self {
        Self {
            sample_rate,
            effective_rate: AtomicU32::new(sample_rate),
            block_size: AtomicUsize::new(block_size),
            dsp,
            soundboard,
            is_running: Arc::new(AtomicBool::new(false)),
            is_muted: Arc::new(AtomicBool::new(false)),
            hear_myself: Arc::new(AtomicBool::new(false)),
            hear_soundboard: Arc::new(AtomicBool::new(true)),
            mic_gain_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            monitor_gain_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            in_peak_bits: Arc::new(AtomicU32::new(0)),
            in_rms_bits: Arc::new(AtomicU32::new(0)),
            out_peak_bits: Arc::new(AtomicU32::new(0)),
            out_rms_bits: Arc::new(AtomicU32::new(0)),
            selected_input_name: Mutex::new(None),
            selected_monitor_name: Mutex::new(None),
            input_stream: Mutex::new(None),
            virtual_stream: Mutex::new(None),
            monitor_stream: Mutex::new(None),
        }
    }

    pub fn get_block_size(&self) -> usize {
        self.block_size.load(Ordering::Relaxed)
    }

    /// Clock the engine is currently running on (see `effective_rate`).
    pub fn get_effective_rate(&self) -> u32 {
        self.effective_rate.load(Ordering::Relaxed)
    }

    pub fn get_mic_gain(&self) -> f32 {
        f32::from_bits(self.mic_gain_bits.load(Ordering::Relaxed))
    }

    pub fn get_monitor_gain(&self) -> f32 {
        f32::from_bits(self.monitor_gain_bits.load(Ordering::Relaxed))
    }

    pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let def_dev = host.default_input_device();
        let def_name = def_dev.as_ref().map(|d| get_device_name(d));
        let def_driver = def_dev
            .as_ref()
            .and_then(|d| d.description().ok())
            .and_then(|desc| desc.driver().map(|s| s.to_string()));

        let mut list = Vec::new();
        if let Ok(devices) = host.input_devices() {
            for dev in devices {
                if is_unusable_or_virtual_device(&dev) {
                    continue;
                }
                let name = get_device_name(&dev);
                let dev_driver = dev
                    .description()
                    .ok()
                    .and_then(|desc| desc.driver().map(|s| s.to_string()));

                let is_default = def_name.as_deref() == Some(&name)
                    || (def_driver.is_some() && def_driver == dev_driver);

                let index = list.len();
                list.push(AudioDeviceInfo {
                    index,
                    name,
                    is_default,
                });
            }
        }

        // If no device was explicitly marked default (e.g. driver name variance),
        // prioritize pipewire / pulse / default or the first device.
        if !list.is_empty() && !list.iter().any(|d| d.is_default) {
            if let Some(pos) = list.iter().position(|d| {
                let n = d.name.to_lowercase();
                n.contains("pipewire") || n.contains("pulse") || n.contains("default")
            }) {
                list[pos].is_default = true;
            } else {
                list[0].is_default = true;
            }
        }

        list
    }

    pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let def_dev = host.default_output_device();
        let def_name = def_dev.as_ref().map(|d| get_device_name(d));
        let def_driver = def_dev
            .as_ref()
            .and_then(|d| d.description().ok())
            .and_then(|desc| desc.driver().map(|s| s.to_string()));

        let mut list = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                if is_unusable_or_virtual_device(&dev) {
                    continue;
                }
                let name = get_device_name(&dev);
                let dev_driver = dev
                    .description()
                    .ok()
                    .and_then(|desc| desc.driver().map(|s| s.to_string()));

                let is_default = def_name.as_deref() == Some(&name)
                    || (def_driver.is_some() && def_driver == dev_driver);

                let index = list.len();
                list.push(AudioDeviceInfo {
                    index,
                    name,
                    is_default,
                });
            }
        }

        if !list.is_empty() && !list.iter().any(|d| d.is_default) {
            if let Some(pos) = list.iter().position(|d| {
                let n = d.name.to_lowercase();
                n.contains("pipewire") || n.contains("pulse") || n.contains("default")
            }) {
                list[pos].is_default = true;
            } else {
                list[0].is_default = true;
            }
        }

        list
    }

    fn collect_input_devices(host: &cpal::Host) -> Vec<Device> {
        host.input_devices().map(|it| it.collect()).unwrap_or_default()
    }

    fn collect_output_devices(host: &cpal::Host) -> Vec<Device> {
        host.output_devices().map(|it| it.collect()).unwrap_or_default()
    }

    /// Resolves the physical microphone, never returning one of Audiover's
    /// own virtual devices or black-hole null devices.
    /// Mirrors Python `resolve_input_device`.
    fn resolve_input_device_named(host: &cpal::Host, saved: Option<&str>) -> Option<Device> {
        let devices = Self::collect_input_devices(host);
        // 1. Saved device name lookup (substring, case-insensitive).
        if let Some(name) = saved {
            if !is_virtual_device_name(name) {
                if let Some(dev) = match_device_by_name(devices.clone(), name) {
                    info!("Restored saved input device: {}", get_device_name(&dev));
                    return Some(dev);
                }
            } else {
                warn!("Ignoring unusable saved input device: {}", name);
            }
        }
        // 2. System default input device (unless unusable/virtual).
        if let Some(def) = host.default_input_device() {
            let name = get_device_name(&def);
            if !is_unusable_or_virtual_device(&def) {
                info!("Using system default input device: {}", name);
                return Some(def);
            }
        }
        // 3. First non-virtual input device.
        for dev in devices {
            if !is_unusable_or_virtual_device(&dev) {
                let name = get_device_name(&dev);
                info!("Fallback selected input device: {}", name);
                return Some(dev);
            }
        }
        None
    }

    /// Resolves the monitor (headphone/speaker) device.
    /// Returns `None` when explicitly disabled (`"none"`) or unavailable.
    /// Mirrors Python `resolve_monitor_device`.
    fn resolve_monitor_device_named(host: &cpal::Host, saved: Option<&str>) -> Option<Device> {
        if saved == Some(MONITOR_DISABLED_SENTINEL) {
            return None;
        }
        let devices = Self::collect_output_devices(host);
        // 1. Saved device name lookup.
        if let Some(name) = saved {
            if !is_virtual_device_name(name) {
                if let Some(dev) = match_device_by_name(devices.clone(), name) {
                    info!("Restored saved monitor device: {}", get_device_name(&dev));
                    return Some(dev);
                }
            } else {
                warn!("Ignoring unusable saved monitor device: {}", name);
            }
        }
        // 2. System default output device (unless unusable/virtual).
        if let Some(def) = host.default_output_device() {
            let name = get_device_name(&def);
            if !is_unusable_or_virtual_device(&def) {
                info!("Using system default output device: {}", name);
                return Some(def);
            }
        }
        // 3. First non-virtual output device.
        for dev in devices {
            if !is_unusable_or_virtual_device(&dev) {
                let name = get_device_name(&dev);
                info!("Fallback selected monitor device: {}", name);
                return Some(dev);
            }
        }
        None
    }

    /// Finds the virtual null sink, retrying briefly since PipeWire may need
    /// a moment after `module-null-sink` is loaded.
    /// Mirrors Python's wait + retry in `start()`.
    pub fn find_virtual_sink_device() -> Option<Device> {
        for attempt in 0..VIRTUAL_SINK_RETRIES {
            if let Some(dev) = Self::find_virtual_sink_once() {
                return Some(dev);
            }
            if attempt + 1 < VIRTUAL_SINK_RETRIES {
                std::thread::sleep(Duration::from_millis(VIRTUAL_SINK_RETRY_DELAY_MS));
            }
        }
        warn!("Audiover virtual sink device not detected.");
        None
    }

    /// Single-pass virtual-sink lookup without retry (for diagnostics).
    pub fn find_virtual_sink_once() -> Option<Device> {
        let host = cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                let name = get_device_name(&dev);
                let lower = name.to_lowercase();
                if (lower.contains("audiover_sink")
                    || lower.contains("audiover_virtual_sink")
                    || lower.contains("audiover"))
                    && !lower.contains("mic")
                    && !lower.contains("source")
                    && !lower.contains("remap")
                {
                    return Some(dev);
                }
            }
        }
        None
    }

    pub fn is_virtual_sink_available() -> bool {
        if Self::find_virtual_sink_once().is_some() {
            return true;
        }
        if let Ok(out) = std::process::Command::new("pactl").args(["list", "short", "sinks"]).output() {
            let s = String::from_utf8_lossy(&out.stdout);
            return s.contains("Audiover_Sink");
        }
        false
    }

    /// Picks the best supported input config: fewest channels (mono
    /// preferred), 48 kHz when supported, float samples when available.
    /// Falls back to the device default instead of failing outright
    /// (a hardcoded config many devices reject = dead microphone).
    /// Returns the negotiated (config, sample format, channels).
    fn pick_input_config(
        device: &Device,
        want_rate: u32,
        block: u32,
    ) -> Option<(StreamConfig, SampleFormat, u16)> {
        let mut ranges: Vec<SupportedStreamConfigRange> = device
            .supported_input_configs()
            .map(|it| it.collect())
            .unwrap_or_default();
        if ranges.is_empty() {
            let def = device.default_input_config().ok()?;
            let format = def.sample_format();
            let channels = def.channels();
            let mut cfg = def.config();
            cfg.buffer_size = BufferSize::Default;
            return Some((cfg, format, channels));
        }
        ranges.sort_by_key(|c| {
            let channels_score = if c.channels() == 1 {
                0
            } else {
                c.channels() as i32
            };
            let format_score = match c.sample_format() {
                SampleFormat::F32 => 0,
                SampleFormat::I16 => 1,
                SampleFormat::U16 => 2,
                _ => 3,
            };
            let rate_score = if c.min_sample_rate() <= want_rate && want_rate <= c.max_sample_rate() {
                0
            } else {
                1
            };
            (rate_score, format_score, channels_score)
        });
        let best = ranges.into_iter().next()?;
        let buf = match best.buffer_size() {
            SupportedBufferSize::Range { min, max } if block >= *min && block <= *max => {
                BufferSize::Fixed(block)
            }
            _ => BufferSize::Default,
        };
        let format = best.sample_format();
        let channels = best.channels();
        let in_range =
            want_rate >= best.min_sample_rate() && want_rate <= best.max_sample_rate();
        let supported = if in_range {
            best.with_sample_rate(want_rate)
        } else {
            best.with_max_sample_rate()
        };
        let mut cfg = supported.config();
        cfg.buffer_size = buf;
        Some((cfg, format, channels))
    }

    /// Picks the best supported stereo output config with the same
    /// prefer-float / prefer-48k policy as the input side.
    fn pick_output_config(
        device: &Device,
        want_rate: u32,
        block: u32,
    ) -> Option<(StreamConfig, SampleFormat)> {
        let mut ranges: Vec<SupportedStreamConfigRange> = device
            .supported_output_configs()
            .map(|it| it.collect())
            .unwrap_or_default();
        if ranges.is_empty() {
            let def = device.default_output_config().ok()?;
            if def.channels() < 2 {
                return None;
            }
            let format = def.sample_format();
            let mut cfg = def.config();
            cfg.channels = 2;
            cfg.buffer_size = BufferSize::Default;
            return Some((cfg, format));
        }
        // Prefer stereo, then any multichannel config.
        ranges.sort_by_key(|c| {
            let channels_score = if c.channels() == 2 {
                0
            } else if c.channels() > 2 {
                1
            } else {
                2
            };
            let format_score = match c.sample_format() {
                SampleFormat::F32 => 0,
                SampleFormat::I16 => 1,
                SampleFormat::U16 => 2,
                _ => 3,
            };
            let rate_score = if c.min_sample_rate() <= want_rate && want_rate <= c.max_sample_rate() {
                0
            } else {
                1
            };
            (rate_score, format_score, channels_score)
        });
        let best = ranges.into_iter().next()?;
        if best.channels() < 2 {
            return None;
        }
        let buf = match best.buffer_size() {
            SupportedBufferSize::Range { min, max } if block >= *min && block <= *max => {
                BufferSize::Fixed(block)
            }
            _ => BufferSize::Default,
        };
        let format = best.sample_format();
        let in_range =
            want_rate >= best.min_sample_rate() && want_rate <= best.max_sample_rate();
        let supported = if in_range {
            best.with_sample_rate(want_rate)
        } else {
            best.with_max_sample_rate()
        };
        let mut cfg = supported.config();
        cfg.channels = 2;
        cfg.buffer_size = buf;
        Some((cfg, format))
    }

    pub fn start(&self) -> Result<(), String> {
        self.stop();

        // Reset DSP internal buffers on start
        self.dsp.lock().reset();

        let block_size = self.get_block_size();
        info!(
            "Starting AudioStreamEngine with sample_rate={}, block_size={}",
            self.sample_rate, block_size
        );
        let host = cpal::default_host();

        // 1. Resolve physical input device (never an unusable or virtual device).
        let saved_in = self.selected_input_name.lock().clone();
        let input_dev = Self::resolve_input_device_named(&host, saved_in.as_deref())
            .ok_or_else(|| "No input device available".to_string())?;
        *self.selected_input_name.lock() = Some(get_device_name(&input_dev));

        // 2. Resolve virtual sink (with retry); missing sink only disables
        //    the virtual-mic path instead of killing the whole engine.
        let virt_sink_dev = Self::find_virtual_sink_device();
        if virt_sink_dev.is_none() {
            warn!("No virtual sink device found via CPAL; will use pacat virtual output.");
        }

        // 3. Resolve monitor device (None = explicitly disabled).
        let saved_mon = self.selected_monitor_name.lock().clone();
        let monitor_dev = Self::resolve_monitor_device_named(&host, saved_mon.as_deref());
        if let Some(ref m_dev) = monitor_dev {
            if saved_mon.as_deref() != Some(MONITOR_DISABLED_SENTINEL) {
                *self.selected_monitor_name.lock() = Some(get_device_name(m_dev));
            }
        }

        // 4. Input stream (negotiated config + sample conversion).
        let (in_config, in_format, in_channels) =
            Self::pick_input_config(&input_dev, self.sample_rate, block_size as u32)
                .ok_or_else(|| {
                    format!(
                        "No supported input config on device {}",
                        get_device_name(&input_dev)
                    )
                })?;

        // 4b. Unify the engine clock to the rate the input device actually
        //     granted. Master engine clock drives DSP and soundboard tracks.
        let in_rate = in_config.sample_rate.max(4000);
        self.effective_rate.store(in_rate, Ordering::SeqCst);
        if in_rate != self.sample_rate {
            warn!(
                "Input device runs at {} Hz (engine nominal {} Hz); retuning DSP and soundboard to the device clock.",
                in_rate, self.sample_rate
            );
        }
        self.dsp.lock().set_sample_rate(in_rate as usize);
        self.soundboard.set_target_sample_rate(in_rate);

        // Resolve output device configs ahead of ring buffer creation to negotiate
        // matching capacities and streaming resamplers.
        let virt_config_opt = virt_sink_dev.as_ref().and_then(|dev| {
            Self::pick_output_config(dev, in_rate, block_size as u32)
        });
        let virt_rate = virt_config_opt
            .as_ref()
            .map(|(cfg, _)| cfg.sample_rate)
            .unwrap_or(in_rate);

        let monitor_config_opt = monitor_dev.as_ref().and_then(|dev| {
            Self::pick_output_config(dev, in_rate, block_size as u32)
        });
        let mon_rate = monitor_config_opt
            .as_ref()
            .map(|(cfg, _)| cfg.sample_rate)
            .unwrap_or(in_rate);

        // Rings hold stereo frames; sized and pre-buffered with a stable ~20-25ms cushion
        // to prevent startup underruns and absorb PipeWire/ALSA scheduling jitter.
        let base_ring_capacity = (block_size * RING_BLOCKS).max(8192);

        let virt_capacity =
            ((base_ring_capacity as f64) * (virt_rate as f64 / in_rate as f64)).ceil() as usize;
        let virt_capacity = virt_capacity.max(8192);
        let (mut virt_prod, virt_cons) = RingBuffer::<[f32; 2]>::new(virt_capacity);

        let mon_capacity =
            ((base_ring_capacity as f64) * (mon_rate as f64 / in_rate as f64)).ceil() as usize;
        let mon_capacity = mon_capacity.max(8192);
        let (mut mon_prod, mon_cons) = RingBuffer::<[f32; 2]>::new(mon_capacity);

        let virt_prefill =
            (((block_size * 4) as f64) * (virt_rate as f64 / in_rate as f64)).ceil() as usize;
        let virt_prefill = virt_prefill.max(1024).min(virt_capacity / 2);
        for _ in 0..virt_prefill {
            let _ = virt_prod.push([0.0, 0.0]);
        }

        let mon_prefill =
            (((block_size * 4) as f64) * (mon_rate as f64 / in_rate as f64)).ceil() as usize;
        let mon_prefill = mon_prefill.max(1024).min(mon_capacity / 2);
        for _ in 0..mon_prefill {
            let _ = mon_prod.push([0.0, 0.0]);
        }

        // Dedicated streaming resamplers handle hardware rate mismatches seamlessly
        let virt_resampler = if virt_config_opt.is_some() && virt_rate != in_rate {
            warn!(
                "Virtual sink negotiated {} Hz vs engine clock {} Hz; activating sinc resampler.",
                virt_rate, in_rate
            );
            StreamingStereoResampler::new(in_rate, virt_rate, block_size)
        } else {
            None
        };

        let mon_resampler = if monitor_config_opt.is_some() && mon_rate != in_rate {
            info!(
                "Monitor device negotiated {} Hz vs engine clock {} Hz; activating sinc resampler.",
                mon_rate, in_rate
            );
            StreamingStereoResampler::new(in_rate, mon_rate, block_size)
        } else {
            None
        };

        let processor = InputProcessor {
            dsp: self.dsp.clone(),
            soundboard: self.soundboard.clone(),
            mic_gain_bits: self.mic_gain_bits.clone(),
            monitor_gain_bits: self.monitor_gain_bits.clone(),
            is_muted: self.is_muted.clone(),
            hear_myself: self.hear_myself.clone(),
            hear_soundboard: self.hear_soundboard.clone(),
            in_peak_bits: self.in_peak_bits.clone(),
            in_rms_bits: self.in_rms_bits.clone(),
            out_peak_bits: self.out_peak_bits.clone(),
            out_rms_bits: self.out_rms_bits.clone(),
            virt_prod,
            mon_prod,
            virt_resampler,
            mon_resampler,
            mono: vec![0.0f32; block_size],
            dsp_out: vec![0.0f32; block_size],
            voice_stereo: vec![[0.0, 0.0]; block_size],
            sb_block: vec![[0.0, 0.0]; block_size],
            mon_scratch: vec![[0.0, 0.0]; block_size],
        };

        let in_err = |err| error!("CPAL Input Stream error: {}", err);
        // CPAL callback sizes are not guaranteed to equal our block size,
        // so process the incoming audio in block-sized chunks.
        let input_stream = match in_format {
            SampleFormat::F32 => {
                let mut proc = processor;
                let mut cvt = Vec::<f32>::new();
                input_dev.build_input_stream(
                    in_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let ch = in_channels.max(1) as usize;
                        let frames = data.len() / ch.max(1);
                        if frames == 0 {
                            return;
                        }
                        if cvt.len() < frames {
                            cvt.resize(frames, 0.0);
                        }
                        for (i, frame) in data.chunks(ch).enumerate().take(frames) {
                            cvt[i] = frame[0];
                        }
                        for chunk in cvt[..frames].chunks(block_size) {
                            proc.handle_mono_block(chunk);
                        }
                    },
                    in_err,
                    None,
                )
            }
            SampleFormat::I16 => {
                let mut proc = processor;
                let mut cvt = Vec::<f32>::new();
                input_dev.build_input_stream(
                    in_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let ch = in_channels.max(1) as usize;
                        let frames = data.len() / ch.max(1);
                        if frames == 0 {
                            return;
                        }
                        if cvt.len() < frames {
                            cvt.resize(frames, 0.0);
                        }
                        for (i, frame) in data.chunks(ch).enumerate().take(frames) {
                            cvt[i] = i16_to_f32(frame[0]);
                        }
                        for chunk in cvt[..frames].chunks(block_size) {
                            proc.handle_mono_block(chunk);
                        }
                    },
                    in_err,
                    None,
                )
            }
            SampleFormat::U16 => {
                let mut proc = processor;
                let mut cvt = Vec::<f32>::new();
                input_dev.build_input_stream(
                    in_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let ch = in_channels.max(1) as usize;
                        let frames = data.len() / ch.max(1);
                        if frames == 0 {
                            return;
                        }
                        if cvt.len() < frames {
                            cvt.resize(frames, 0.0);
                        }
                        for (i, frame) in data.chunks(ch).enumerate().take(frames) {
                            cvt[i] = u16_to_f32(frame[0]);
                        }
                        for chunk in cvt[..frames].chunks(block_size) {
                            proc.handle_mono_block(chunk);
                        }
                    },
                    in_err,
                    None,
                )
            }
            other => {
                return Err(format!(
                    "Unsupported input sample format ({}) on device {}",
                    other,
                    get_device_name(&input_dev)
                ));
            }
        }
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

        // 5. Virtual sink output stream (stereo):
        // Prefer CPAL if a supported virtual sink device is exposed by the host audio backend;
        // otherwise stream directly into Audiover_Sink via pacat (PulseAudio / PipeWire).
        let mut virtual_stream_opt: Option<VirtualOutputStream> = None;
        if let Some(v_dev) = virt_sink_dev {
            if let Some((v_config, v_format)) = virt_config_opt {
                let virt_err = |err| error!("CPAL Virtual Output Stream error: {}", err);
                let build_res: Result<Stream, String> = match v_format {
                    SampleFormat::F32 => v_dev
                        .build_output_stream(
                            v_config,
                            make_output_callback_f32(virt_cons, virt_capacity, block_size),
                            virt_err,
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    SampleFormat::I16 => v_dev
                        .build_output_stream(
                            v_config,
                            make_output_callback_i16(virt_cons, virt_capacity, block_size),
                            virt_err,
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    SampleFormat::U16 => v_dev
                        .build_output_stream(
                            v_config,
                            make_output_callback_u16(virt_cons, virt_capacity, block_size),
                            virt_err,
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    other => Err(format!("Unsupported virtual-sink format ({})", other)),
                };
                match build_res {
                    Ok(s) => {
                        if let Err(e) = s.play() {
                            warn!("Could not play virtual sink stream: {}", e);
                        } else {
                            info!(
                                "Started Virtual Sink Output Stream on {}",
                                get_device_name(&v_dev)
                            );
                            virtual_stream_opt = Some(VirtualOutputStream::Cpal(SafeStream(s)));
                        }
                    }
                    Err(e) => warn!("Could not open virtual sink stream: {}", e),
                }
            } else {
                warn!("Virtual sink device has no supported stereo output config.");
                match PacatStream::spawn("Audiover_Sink", in_rate, block_size, virt_cons) {
                    Ok(pacat) => {
                        info!("Started Pacat Virtual Sink Stream on Audiover_Sink");
                        virtual_stream_opt = Some(VirtualOutputStream::Pacat(pacat));
                    }
                    Err(e) => {
                        warn!("Could not open virtual sink stream via pacat: {}", e);
                    }
                }
            }
        } else {
            match PacatStream::spawn("Audiover_Sink", in_rate, block_size, virt_cons) {
                Ok(pacat) => {
                    info!("Started Pacat Virtual Sink Stream on Audiover_Sink");
                    virtual_stream_opt = Some(VirtualOutputStream::Pacat(pacat));
                }
                Err(e) => {
                    warn!("Could not open virtual sink stream via pacat: {}", e);
                }
            }
        }

        // 6. Monitor headphone output stream (stereo):
        // If monitor hardware rate differs from engine clock, streaming resampler
        // in InputProcessor bridges the rate smoothly without stutter or dropouts.
        let mut monitor_stream_opt = None;
        if let Some(m_dev) = monitor_dev {
            if let Some((m_config, m_format)) = monitor_config_opt {
                let mon_err = |err| error!("CPAL Monitor Output Stream error: {}", err);
                let build_res: Result<Stream, String> = match m_format {
                    SampleFormat::F32 => m_dev
                        .build_output_stream(
                            m_config,
                            make_output_callback_f32(mon_cons, mon_capacity, block_size),
                            mon_err,
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    SampleFormat::I16 => m_dev
                        .build_output_stream(
                            m_config,
                            make_output_callback_i16(mon_cons, mon_capacity, block_size),
                            mon_err,
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    SampleFormat::U16 => m_dev
                        .build_output_stream(
                            m_config,
                            make_output_callback_u16(mon_cons, mon_capacity, block_size),
                            mon_err,
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    other => Err(format!("Unsupported monitor format ({})", other)),
                };
                match build_res {
                    Ok(s) => {
                        if let Err(e) = s.play() {
                            warn!("Could not play monitor stream: {}", e);
                        } else {
                            info!(
                                "Started Monitor Output Stream on {}",
                                get_device_name(&m_dev)
                            );
                            monitor_stream_opt = Some(SafeStream(s));
                        }
                    }
                    Err(e) => warn!("Could not open monitor device: {}", e),
                }
            } else {
                warn!("Monitor device has no supported stereo output config.");
            }
        } else {
            info!("Monitor output disabled.");
        }

        input_stream.play().map_err(|e| e.to_string())?;
        info!(
            "Started Input Stream on {}",
            get_device_name(&input_dev)
        );

        *self.input_stream.lock() = Some(SafeStream(input_stream));
        *self.virtual_stream.lock() = virtual_stream_opt;
        *self.monitor_stream.lock() = monitor_stream_opt;
        self.is_running.store(true, Ordering::SeqCst);

        info!("AudioStreamEngine started successfully.");
        Ok(())
    }

    pub fn stop(&self) {
        *self.input_stream.lock() = None;
        *self.virtual_stream.lock() = None;
        *self.monitor_stream.lock() = None;
        self.is_running.store(false, Ordering::SeqCst);
        self.in_peak_bits.store(0, Ordering::Relaxed);
        self.in_rms_bits.store(0, Ordering::Relaxed);
        self.out_peak_bits.store(0, Ordering::Relaxed);
        self.out_rms_bits.store(0, Ordering::Relaxed);
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::SeqCst);
    }

    pub fn set_hear_myself(&self, enabled: bool) {
        self.hear_myself.store(enabled, Ordering::SeqCst);
        // If sidetone was enabled but the monitor stream is missing
        // (e.g. the device was busy at start), retry opening it.
        if enabled && self.is_running.load(Ordering::SeqCst) && self.monitor_stream.lock().is_none()
        {
            let saved = self.selected_monitor_name.lock().clone();
            if saved.as_deref() != Some(MONITOR_DISABLED_SENTINEL) {
                let _ = self.start();
            }
        }
    }

    pub fn set_hear_soundboard(&self, enabled: bool) {
        self.hear_soundboard.store(enabled, Ordering::SeqCst);
    }

    pub fn set_mic_gain(&self, gain: f32) {
        self.mic_gain_bits.store(gain.to_bits(), Ordering::Relaxed);
    }

    pub fn set_monitor_gain(&self, gain: f32) {
        self.monitor_gain_bits.store(gain.to_bits(), Ordering::Relaxed);
    }

    pub fn set_buffer_size(&self, size: usize) {
        let clamped = size.clamp(64, 4096);
        self.block_size.store(clamped, Ordering::SeqCst);
        if self.is_running.load(Ordering::SeqCst) {
            let _ = self.start();
        }
    }

    pub fn set_input_device(&self, index: usize) {
        let inputs = Self::list_input_devices();
        if let Some(dev) = inputs.into_iter().find(|d| d.index == index) {
            *self.selected_input_name.lock() = Some(dev.name);
            if self.is_running.load(Ordering::SeqCst) {
                let _ = self.start();
            }
        }
    }

    /// `None` disables the monitor output (Python `"none"`).
    pub fn set_monitor_device(&self, index: Option<usize>) {
        if let Some(idx) = index {
            let outputs = Self::list_output_devices();
            if let Some(dev) = outputs.into_iter().find(|d| d.index == idx) {
                *self.selected_monitor_name.lock() = Some(dev.name);
            }
        } else {
            *self.selected_monitor_name.lock() = Some(MONITOR_DISABLED_SENTINEL.to_string());
        }
        if self.is_running.load(Ordering::SeqCst) {
            let _ = self.start();
        }
    }

    /// Restores a persisted input device by name (Python parity).
    pub fn set_input_device_name(&self, name: &str) {
        if !is_virtual_device_name(name) {
            *self.selected_input_name.lock() = Some(name.to_string());
        } else {
            *self.selected_input_name.lock() = None;
        }
    }

    /// Restores a persisted monitor device by name (`"none"` disables).
    pub fn set_monitor_device_name(&self, name: &str) {
        if name == MONITOR_DISABLED_SENTINEL || !is_virtual_device_name(name) {
            *self.selected_monitor_name.lock() = Some(name.to_string());
        } else {
            *self.selected_monitor_name.lock() = None;
        }
    }

    pub fn selected_input_name(&self) -> Option<String> {
        self.selected_input_name.lock().clone()
    }

    pub fn selected_monitor_name(&self) -> Option<String> {
        self.selected_monitor_name.lock().clone()
    }

    pub fn get_devices_state(&self) -> AudioDevicesState {
        let inputs = Self::list_input_devices();
        let outputs = Self::list_output_devices();
        let host = cpal::default_host();

        let sel_in = self.selected_input_name.lock().clone();
        let current_input = inputs
            .iter()
            .find(|d| {
                sel_in.as_deref().map(|s| {
                    let a = d.name.to_lowercase();
                    let b = s.trim().to_lowercase();
                    (a == b || a.contains(b.as_str())) && !is_virtual_device_name(&d.name)
                }).unwrap_or(false)
            })
            .map(|d| d.index)
            .or_else(|| {
                Self::resolve_input_device_named(&host, sel_in.as_deref()).and_then(|dev| {
                    let name = get_device_name(&dev);
                    inputs.iter().find(|d| d.name == name).map(|d| d.index)
                })
            })
            .or_else(|| {
                inputs.iter().find(|d| d.is_default).map(|d| d.index)
            })
            .or_else(|| {
                inputs.first().map(|d| d.index)
            });

        let sel_mon = self.selected_monitor_name.lock().clone();
        let current_monitor = if sel_mon.as_deref() == Some(MONITOR_DISABLED_SENTINEL) {
            None
        } else {
            outputs
                .iter()
                .find(|d| {
                    sel_mon.as_deref().map(|s| {
                        let a = d.name.to_lowercase();
                        let b = s.trim().to_lowercase();
                        (a == b || a.contains(b.as_str())) && !is_virtual_device_name(&d.name)
                    }).unwrap_or(false)
                })
                .map(|d| d.index)
                .or_else(|| {
                    Self::resolve_monitor_device_named(&host, sel_mon.as_deref()).and_then(
                        |dev| {
                            let name = get_device_name(&dev);
                            outputs.iter().find(|d| d.name == name).map(|d| d.index)
                        },
                    )
                })
                .or_else(|| {
                    outputs.iter().find(|d| d.is_default).map(|d| d.index)
                })
                .or_else(|| {
                    outputs.first().map(|d| d.index)
                })
        };

        AudioDevicesState {
            inputs,
            outputs,
            current_input,
            current_monitor,
            block_size: self.get_block_size(),
            mic_gain: self.get_mic_gain(),
            monitor_gain: self.get_monitor_gain(),
            hear_myself: self.hear_myself.load(Ordering::Relaxed),
            hear_soundboard: self.hear_soundboard.load(Ordering::Relaxed),
        }
    }

    pub fn get_meters(&self) -> Meters {
        Meters {
            in_peak: f32::from_bits(self.in_peak_bits.load(Ordering::Relaxed)),
            in_rms: f32::from_bits(self.in_rms_bits.load(Ordering::Relaxed)),
            out_peak: f32::from_bits(self.out_peak_bits.load(Ordering::Relaxed)),
            out_rms: f32::from_bits(self.out_rms_bits.load(Ordering::Relaxed)),
            is_muted: self.is_muted.load(Ordering::Relaxed),
            hear_myself: self.hear_myself.load(Ordering::Relaxed),
            engine_active: self.is_running.load(Ordering::Relaxed),
        }
    }
}

pub struct PacatStream {
    running: Arc<AtomicBool>,
    child: Arc<Mutex<std::process::Child>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl PacatStream {
    pub fn spawn(
        sink_name: &str,
        sample_rate: u32,
        block_size: usize,
        mut consumer: rtrb::Consumer<[f32; 2]>,
    ) -> Result<Self, String> {
        let mut child = std::process::Command::new("pacat")
            .args([
                "--playback",
                "-d",
                sink_name,
                "--format=float32le",
                &format!("--rate={}", sample_rate),
                "--channels=2",
                "--latency-msec=25",
                "--raw",
                "--stream-name=Audiover_Virtual_Mic",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn pacat: {}", e))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture pacat stdin".to_string())?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let worker = std::thread::Builder::new()
            .name("audiover-virt-pacat".into())
            .spawn(move || {
                use std::io::Write;
                let chunk_limit = block_size.max(256);
                let mut byte_buf = Vec::<u8>::with_capacity(chunk_limit * 8);
                let mut last_sample = [0.0f32, 0.0];

                // Wait for initial pre-buffering cushion before streaming to pacat
                let mut waits = 0;
                while running_clone.load(Ordering::Relaxed) && consumer.slots() < block_size && waits < 50 {
                    std::thread::sleep(Duration::from_millis(2));
                    waits += 1;
                }

                while running_clone.load(Ordering::Relaxed) {
                    byte_buf.clear();
                    let mut count = 0;
                    while count < chunk_limit {
                        match consumer.pop() {
                            Ok(frame) => {
                                last_sample = frame;
                                byte_buf.extend_from_slice(&frame[0].to_le_bytes());
                                byte_buf.extend_from_slice(&frame[1].to_le_bytes());
                                count += 1;
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }

                    if count > 0 {
                        if stdin.write_all(&byte_buf).is_err() {
                            break;
                        }
                    } else {
                        // Soft decay to avoid harsh click on temporary underrun
                        if last_sample[0].abs() > 0.001 || last_sample[1].abs() > 0.001 {
                            last_sample[0] *= 0.85;
                            last_sample[1] *= 0.85;
                            let b0 = last_sample[0].to_le_bytes();
                            let b1 = last_sample[1].to_le_bytes();
                            let _ = stdin.write_all(&[b0[0], b0[1], b0[2], b0[3], b1[0], b1[1], b1[2], b1[3]]);
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn pacat worker: {}", e))?;

        Ok(Self {
            running,
            child: Arc::new(Mutex::new(child)),
            worker: Some(worker),
        })
    }
}

impl Drop for PacatStream {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let mut child = self.child.lock();
        let _ = child.kill();
        let _ = child.wait();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[allow(dead_code)]
pub enum VirtualOutputStream {
    Cpal(SafeStream),
    Pacat(PacatStream),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_sink_detection() {
        let _ = AudioStreamEngine::is_virtual_sink_available();
    }

    #[test]
    fn test_is_virtual_device_name() {
        assert!(is_virtual_device_name("Audiover_Sink"));
        assert!(is_virtual_device_name("Audiover_Mic"));
        assert!(is_virtual_device_name("Audiover_Virtual_Sink"));
        assert!(is_virtual_device_name("module-null_sink"));
        assert!(is_virtual_device_name("Discard all samples (playback) or generate zero samples (capture)"));
        assert!(is_virtual_device_name("null"));
        assert!(is_virtual_device_name("dummy output"));
        assert!(!is_virtual_device_name("Realtek ALC256"));
        assert!(!is_virtual_device_name("HDA Intel PCH"));
        assert!(!is_virtual_device_name("PipeWire Sound Server"));
        assert!(!is_virtual_device_name("Default ALSA Output (currently PipeWire Media Server)"));
        assert!(!is_virtual_device_name("USB Audio Device"));
    }

    #[test]
    fn test_device_listing_filters_unusable() {
        let inputs = AudioStreamEngine::list_input_devices();
        for dev in &inputs {
            assert!(
                !is_virtual_device_name(&dev.name),
                "Device {} should have been filtered out",
                dev.name
            );
        }
        if !inputs.is_empty() {
            assert!(
                inputs.iter().any(|d| d.is_default),
                "At least one input device should be marked default"
            );
            // Verify indices are contiguous 0..N
            for (i, dev) in inputs.iter().enumerate() {
                assert_eq!(dev.index, i);
            }
        }

        let outputs = AudioStreamEngine::list_output_devices();
        for dev in &outputs {
            assert!(
                !is_virtual_device_name(&dev.name),
                "Device {} should have been filtered out",
                dev.name
            );
        }
        if !outputs.is_empty() {
            assert!(
                outputs.iter().any(|d| d.is_default),
                "At least one output device should be marked default"
            );
            for (i, dev) in outputs.iter().enumerate() {
                assert_eq!(dev.index, i);
            }
        }
    }

    #[test]
    fn test_pacat_stream_lifecycle() {
        let (mut prod, cons) = RingBuffer::<[f32; 2]>::new(1024);
        for _ in 0..512 {
            let _ = prod.push([0.1, 0.1]);
        }
        if AudioStreamEngine::is_virtual_sink_available() {
            let stream = PacatStream::spawn("Audiover_Sink", 48000, 256, cons);
            assert!(stream.is_ok());
            std::thread::sleep(Duration::from_millis(50));
            // Stream should drop cleanly without hanging
            drop(stream);
        }
    }

    #[test]
    fn test_streaming_stereo_resampler() {
        let mut resampler =
            StreamingStereoResampler::new(16000, 48000, 256).expect("resampler creation");
        let (mut prod, cons) = RingBuffer::<[f32; 2]>::new(4096);
        let frames = vec![[0.5, -0.5]; 256];
        resampler.resample_and_push(&frames, &mut prod);
        assert!(cons.slots() > 0);
        // First block produces ~762 frames due to initial sinc filter group delay
        let first_count = cons.slots();
        assert!(
            (first_count as i32 - 768).abs() <= 10,
            "Expected ~768 frames, got {}",
            first_count
        );

        // Subsequent blocks produce exactly 768 frames
        resampler.resample_and_push(&frames, &mut prod);
        let total_count = cons.slots();
        let second_chunk = total_count - first_count;
        assert_eq!(second_chunk, 768);
    }
}

