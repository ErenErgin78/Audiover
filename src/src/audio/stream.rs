use super::dsp::VoiceDSP;
use crate::soundboard::player::SoundboardPlayer;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use log::{error, info};
use parking_lot::Mutex;
use rtrb::RingBuffer;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

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

pub struct AudioStreamEngine {
    pub sample_rate: u32,
    pub block_size: usize,
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
    virtual_stream: Mutex<Option<SafeStream>>,
    monitor_stream: Mutex<Option<SafeStream>>,
}

fn get_device_name(dev: &Device) -> String {
    dev.description()
        .ok()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|| dev.to_string())
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
            block_size,
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

    pub fn get_mic_gain(&self) -> f32 {
        f32::from_bits(self.mic_gain_bits.load(Ordering::Relaxed))
    }

    pub fn get_monitor_gain(&self) -> f32 {
        f32::from_bits(self.monitor_gain_bits.load(Ordering::Relaxed))
    }

    pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let default_name = host.default_input_device().map(|d| get_device_name(&d));
        let mut list = Vec::new();
        if let Ok(devices) = host.input_devices() {
            for (idx, dev) in devices.enumerate() {
                let name = get_device_name(&dev);
                let is_default = default_name.as_deref() == Some(&name);
                list.push(AudioDeviceInfo {
                    index: idx,
                    name,
                    is_default,
                });
            }
        }
        list
    }

    pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let default_name = host.default_output_device().map(|d| get_device_name(&d));
        let mut list = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for (idx, dev) in devices.enumerate() {
                let name = get_device_name(&dev);
                let is_default = default_name.as_deref() == Some(&name);
                list.push(AudioDeviceInfo {
                    index: idx,
                    name,
                    is_default,
                });
            }
        }
        list
    }

    pub fn find_virtual_sink_device() -> Option<Device> {
        let host = cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                let name = get_device_name(&dev);
                let lower = name.to_lowercase();
                if (lower.contains("audiover_sink") || lower.contains("audiover_virtual_sink") || lower.contains("audiover"))
                    && !lower.contains("mic")
                    && !lower.contains("remap")
                {
                    return Some(dev);
                }
            }
        }
        None
    }

    pub fn start(&self) -> Result<(), String> {
        self.stop();

        // Reset DSP internal buffers on start
        self.dsp.lock().reset();

        info!("Starting AudioStreamEngine with sample_rate={}, block_size={}", self.sample_rate, self.block_size);
        let host = cpal::default_host();

        // 1. Resolve Input Device
        let input_dev = {
            let selected = self.selected_input_name.lock().clone();
            if let Some(name) = selected {
                host.input_devices()
                    .ok()
                    .and_then(|mut devs| devs.find(|d| get_device_name(d) == name))
                    .or_else(|| host.default_input_device())
            } else {
                host.default_input_device()
            }
        }.ok_or_else(|| "No input device available".to_string())?;

        // 2. Resolve Virtual Sink Device
        let virt_sink_dev = Self::find_virtual_sink_device();

        // 3. Resolve Monitor Device (Headphones / Speakers)
        let monitor_dev = {
            let selected = self.selected_monitor_name.lock().clone();
            if let Some(name) = selected {
                host.output_devices()
                    .ok()
                    .and_then(|mut devs| devs.find(|d| get_device_name(d) == name))
                    .or_else(|| host.default_output_device())
            } else {
                host.default_output_device()
            }
        };

        let ring_capacity = self.block_size * 16;
        let (mut virt_prod, mut virt_cons) = RingBuffer::<f32>::new(ring_capacity);
        let (mut mon_prod, mut mon_cons) = RingBuffer::<f32>::new(ring_capacity);

        // Input Stream Setup
        let in_config = StreamConfig {
            channels: 1,
            sample_rate: self.sample_rate,
            buffer_size: cpal::BufferSize::Fixed(self.block_size as u32),
        };

        let dsp_ref = self.dsp.clone();
        let mic_gain_ref = self.mic_gain_bits.clone();
        let is_muted_ref = self.is_muted.clone();
        let in_peak_ref = self.in_peak_bits.clone();
        let in_rms_ref = self.in_rms_bits.clone();
        let block_sz = self.block_size;

        let mut in_scratch = vec![0.0f32; block_sz];
        let mut dsp_out_scratch = vec![0.0f32; block_sz];

        let in_err = |err| error!("CPAL Input Stream error: {}", err);
        let input_stream = input_dev.build_input_stream(
            in_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let n = data.len().min(block_sz);
                if n == 0 {
                    return;
                }

                let is_muted = is_muted_ref.load(Ordering::Relaxed);
                let mic_gain = f32::from_bits(mic_gain_ref.load(Ordering::Relaxed));
                let mut peak = 0.0f32;
                let mut sum_sq = 0.0f32;

                for (i, &sample) in data.iter().enumerate().take(n) {
                    let s = if is_muted { 0.0 } else { sample * mic_gain };
                    in_scratch[i] = s;
                    let abs_s = s.abs();
                    if abs_s > peak {
                        peak = abs_s;
                    }
                    sum_sq += s * s;
                }

                in_peak_ref.store(peak.to_bits(), Ordering::Relaxed);
                let rms = (sum_sq / (n as f32).max(1.0)).sqrt();
                in_rms_ref.store(rms.to_bits(), Ordering::Relaxed);

                dsp_ref.lock().process(&in_scratch[..n], &mut dsp_out_scratch[..n]);

                for &s in &dsp_out_scratch[..n] {
                    let _ = virt_prod.push(s);
                    let _ = mon_prod.push(s);
                }
            },
            in_err,
            None,
        ).map_err(|e| format!("Failed to build input stream: {}", e))?;

        // Virtual Sink Output Stream Setup (Stereo)
        let out_config = StreamConfig {
            channels: 2,
            sample_rate: self.sample_rate,
            buffer_size: cpal::BufferSize::Fixed(self.block_size as u32),
        };

        let mut virtual_stream_opt = None;
        if let Some(v_dev) = virt_sink_dev {
            let sb_ref = self.soundboard.clone();
            let virt_err = |err| error!("CPAL Virtual Output Stream error: {}", err);
            let mut stereo_scratch: Vec<[f32; 2]> = vec![[0.0, 0.0]; block_sz];
            let out_peak_ref = self.out_peak_bits.clone();
            let out_rms_ref = self.out_rms_bits.clone();

            let v_stream_res = v_dev.build_output_stream(
                out_config.clone(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let num_frames = data.len() / 2;
                    if stereo_scratch.len() < num_frames {
                        stereo_scratch.resize(num_frames, [0.0, 0.0]);
                    }

                    for frame in stereo_scratch.iter_mut().take(num_frames) {
                        let s = virt_cons.pop().unwrap_or(0.0);
                        *frame = [s, s];
                    }

                    sb_ref.mix_into(&mut stereo_scratch[..num_frames], 1.0);

                    let mut peak = 0.0f32;
                    let mut sum_sq = 0.0f32;

                    for (i, frame) in stereo_scratch.iter().enumerate().take(num_frames) {
                        data[i * 2] = frame[0];
                        data[i * 2 + 1] = frame[1];
                        let abs_l = frame[0].abs();
                        let abs_r = frame[1].abs();
                        if abs_l > peak { peak = abs_l; }
                        if abs_r > peak { peak = abs_r; }
                        sum_sq += abs_l * abs_l + abs_r * abs_r;
                    }

                    out_peak_ref.store(peak.to_bits(), Ordering::Relaxed);
                    let rms = (sum_sq / ((num_frames * 2) as f32).max(1.0)).sqrt();
                    out_rms_ref.store(rms.to_bits(), Ordering::Relaxed);
                },
                virt_err,
                None,
            );

            if let Ok(s) = v_stream_res {
                let _ = s.play();
                virtual_stream_opt = Some(SafeStream(s));
            }
        }

        // Monitor Output Stream Setup (Headphones)
        let mut monitor_stream_opt = None;
        if let Some(m_dev) = monitor_dev {
            let sb_mon_ref = self.soundboard.clone();
            let hear_voice_ref = self.hear_myself.clone();
            let is_muted_mon_ref = self.is_muted.clone();
            let hear_sb_ref = self.hear_soundboard.clone();
            let mon_gain_ref = self.monitor_gain_bits.clone();

            let mon_err = |err| error!("CPAL Monitor Output Stream error: {}", err);
            let mut mon_stereo_scratch: Vec<[f32; 2]> = vec![[0.0, 0.0]; block_sz];

            let mon_s = m_dev.build_output_stream(
                out_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let num_frames = data.len() / 2;
                    if mon_stereo_scratch.len() < num_frames {
                        mon_stereo_scratch.resize(num_frames, [0.0, 0.0]);
                    }

                    let is_muted = is_muted_mon_ref.load(Ordering::Relaxed);
                    let hear_voice = hear_voice_ref.load(Ordering::Relaxed) && !is_muted;
                    let hear_sb = hear_sb_ref.load(Ordering::Relaxed);
                    let mon_gain = f32::from_bits(mon_gain_ref.load(Ordering::Relaxed));

                    for frame in mon_stereo_scratch.iter_mut().take(num_frames) {
                        let s = mon_cons.pop().unwrap_or(0.0);
                        if hear_voice {
                            *frame = [s, s];
                        } else {
                            *frame = [0.0, 0.0];
                        }
                    }

                    if hear_sb {
                        sb_mon_ref.mix_into(&mut mon_stereo_scratch[..num_frames], 1.0);
                    }

                    for (i, frame) in mon_stereo_scratch.iter().enumerate().take(num_frames) {
                        data[i * 2] = frame[0] * mon_gain;
                        data[i * 2 + 1] = frame[1] * mon_gain;
                    }
                },
                mon_err,
                None,
            );

            if let Ok(s) = mon_s {
                let _ = s.play();
                monitor_stream_opt = Some(SafeStream(s));
            }
        }

        input_stream.play().map_err(|e| e.to_string())?;

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
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::SeqCst);
    }

    pub fn set_hear_myself(&self, enabled: bool) {
        self.hear_myself.store(enabled, Ordering::SeqCst);
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

    pub fn set_buffer_size(&self, _size: usize) {
        let _ = self.start();
    }

    pub fn set_input_device(&self, index: usize) {
        let inputs = Self::list_input_devices();
        if let Some(dev) = inputs.into_iter().find(|d| d.index == index) {
            *self.selected_input_name.lock() = Some(dev.name);
            let _ = self.start();
        }
    }

    pub fn set_monitor_device(&self, index: Option<usize>) {
        if let Some(idx) = index {
            let outputs = Self::list_output_devices();
            if let Some(dev) = outputs.into_iter().find(|d| d.index == idx) {
                *self.selected_monitor_name.lock() = Some(dev.name);
            }
        } else {
            *self.selected_monitor_name.lock() = None;
        }
        let _ = self.start();
    }

    pub fn get_devices_state(&self) -> AudioDevicesState {
        let inputs = Self::list_input_devices();
        let outputs = Self::list_output_devices();

        let sel_in = self.selected_input_name.lock().clone();
        let current_input = inputs
            .iter()
            .find(|d| sel_in.as_deref() == Some(&d.name) || (sel_in.is_none() && d.is_default))
            .map(|d| d.index);

        let sel_mon = self.selected_monitor_name.lock().clone();
        let current_monitor = outputs
            .iter()
            .find(|d| sel_mon.as_deref() == Some(&d.name))
            .map(|d| d.index);

        AudioDevicesState {
            inputs,
            outputs,
            current_input,
            current_monitor,
            block_size: self.block_size,
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

