use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DSPOptions {
    pub bypass: bool,
    pub noise_gate_enabled: bool,
    pub noise_gate_threshold_db: f32,

    pub pitch_semitones: f32,

    pub robot_enabled: bool,
    pub robot_freq: f32,
    pub robot_mix: f32,

    pub radio_enabled: bool,

    pub distortion_enabled: bool,
    pub distortion_drive: f32,

    pub reverb_enabled: bool,
    pub reverb_room_size: f32,
    pub reverb_wet: f32,

    pub chorus_enabled: bool,
    pub chorus_depth: f32,
    pub chorus_rate: f32,

    pub highpass_cutoff: f32,
    pub lowpass_cutoff: f32,

    pub input_gain: f32,
    pub output_gain: f32,
}

impl Default for DSPOptions {
    fn default() -> Self {
        Self {
            bypass: false,
            noise_gate_enabled: false,
            noise_gate_threshold_db: -65.0,

            pitch_semitones: 0.0,

            robot_enabled: false,
            robot_freq: 150.0,
            robot_mix: 0.75,

            radio_enabled: false,

            distortion_enabled: false,
            distortion_drive: 0.0,

            reverb_enabled: false,
            reverb_room_size: 0.0,
            reverb_wet: 0.0,

            chorus_enabled: false,
            chorus_depth: 0.0,
            chorus_rate: 1.2,

            highpass_cutoff: 20.0,
            lowpass_cutoff: 20000.0,

            input_gain: 1.0,
            output_gain: 1.0,
        }
    }
}

/// High-performance real-time vocal pitch shifter using 4-grain synthesis.
#[derive(Debug, Clone)]
pub struct ContinuousPitchShifter {
    sr: usize,
    buf_len: usize,
    buffer: Vec<f32>,
    write_ptr: usize,
    phases: [f32; 4],
}

impl ContinuousPitchShifter {
    pub fn new(sample_rate: usize) -> Self {
        let buf_len = sample_rate / 4; // 250ms circular buffer
        Self {
            sr: sample_rate,
            buf_len,
            buffer: vec![0.0; buf_len],
            write_ptr: 0,
            phases: [0.0, 0.25, 0.5, 0.75],
        }
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_ptr = 0;
        self.phases = [0.0, 0.25, 0.5, 0.75];
    }

    pub fn process(&mut self, audio: &[f32], semitones: f32, out: &mut [f32]) {
        if semitones.abs() < 0.05 {
            out.copy_from_slice(audio);
            return;
        }

        let pitch_ratio = 2.0_f32.powf(semitones / 12.0);
        let rate = 1.0 - pitch_ratio;

        let window_ms = if semitones < 0.0 {
            35.0 + (semitones.abs() * 1.5).min(25.0)
        } else {
            (20.0 - semitones * 0.6).max(12.0)
        };

        let w_size = ((self.sr as f32) * (window_ms / 1000.0)).max(16.0);
        let n = audio.len();
        let buf_len = self.buf_len;

        // Zero output buffer
        out.fill(0.0);

        for (i, &sample) in audio.iter().enumerate() {
            let curr_ptr = self.write_ptr + i;
            let write_idx = curr_ptr % buf_len;
            self.buffer[write_idx] = sample;

            let step = (i as f32) * (rate / w_size);
            let mut sum_sample = 0.0;

            for k in 0..4 {
                let phase_k = (self.phases[k] + step).rem_euclid(1.0);
                // Hann window
                let w = 0.5 * (1.0 - (2.0 * PI * phase_k).cos());
                let delay_samples = phase_k * w_size;
                let tap = (curr_ptr as f32) - delay_samples;

                let tap_floor = tap.floor();
                let idx0 = ((tap_floor as isize).rem_euclid(buf_len as isize)) as usize;
                let idx1 = (idx0 + 1) % buf_len;
                let frac = tap - tap_floor;

                let grain_val = (1.0 - frac) * self.buffer[idx0] + frac * self.buffer[idx1];
                sum_sample += w * grain_val;
            }

            out[i] = sum_sample * 0.5;
        }

        let total_step = (n as f32) * (rate / w_size);
        for k in 0..4 {
            self.phases[k] = (self.phases[k] + total_step).rem_euclid(1.0);
        }
        self.write_ptr = (self.write_ptr + n) % buf_len;
    }
}

/// Low-latency Schroeder Reverb with 4 comb filters and 2 all-pass diffusers.
#[derive(Debug, Clone)]
pub struct SchroederReverb {
    comb_buffers: Vec<Vec<f32>>,
    comb_ptrs: Vec<usize>,
    ap_buffers: Vec<Vec<f32>>,
    ap_ptrs: Vec<usize>,
    wet_temp: Vec<f32>,
}

impl SchroederReverb {
    pub fn new(sample_rate: usize) -> Self {
        let comb_times = [0.0297, 0.0371, 0.0411, 0.0437];
        let ap_times = [0.0050, 0.0017];

        let comb_buffers: Vec<Vec<f32>> = comb_times
            .iter()
            .map(|&t| vec![0.0; ((sample_rate as f64) * t).round() as usize])
            .collect();
        let comb_ptrs = vec![0; comb_buffers.len()];

        let ap_buffers: Vec<Vec<f32>> = ap_times
            .iter()
            .map(|&t| vec![0.0; ((sample_rate as f64) * t).round() as usize])
            .collect();
        let ap_ptrs = vec![0; ap_buffers.len()];

        Self {
            comb_buffers,
            comb_ptrs,
            ap_buffers,
            ap_ptrs,
            wet_temp: Vec::with_capacity(1024),
        }
    }

    pub fn reset(&mut self) {
        for b in &mut self.comb_buffers {
            b.fill(0.0);
        }
        for b in &mut self.ap_buffers {
            b.fill(0.0);
        }
        self.comb_ptrs.fill(0);
        self.ap_ptrs.fill(0);
    }

    pub fn process(&mut self, audio: &[f32], room_size: f32, wet: f32, out: &mut [f32]) {
        if wet <= 0.001 {
            out.copy_from_slice(audio);
            return;
        }

        let n = audio.len();
        if self.wet_temp.len() < n {
            self.wet_temp.resize(n, 0.0);
        }
        self.wet_temp[..n].fill(0.0);

        let feedback = 0.65 + 0.28 * room_size.clamp(0.0, 1.0);

        // 4 Parallel Comb Filters
        for (c_idx, buf) in self.comb_buffers.iter_mut().enumerate() {
            let b_len = buf.len();
            let mut ptr = self.comb_ptrs[c_idx];
            for (i, &sample) in audio.iter().enumerate().take(n) {
                let delayed = buf[ptr];
                buf[ptr] = sample + delayed * feedback;
                self.wet_temp[i] += delayed * 0.25;
                ptr = (ptr + 1) % b_len;
            }
            self.comb_ptrs[c_idx] = ptr;
        }

        // 2 Series All-Pass Filters
        let ap_gain = 0.5;
        for (ap_idx, buf) in self.ap_buffers.iter_mut().enumerate() {
            let b_len = buf.len();
            let mut ptr = self.ap_ptrs[ap_idx];
            for i in 0..n {
                let delayed = buf[ptr];
                let in_val = self.wet_temp[i];
                buf[ptr] = in_val + delayed * ap_gain;
                self.wet_temp[i] = delayed - in_val * ap_gain;
                ptr = (ptr + 1) % b_len;
            }
            self.ap_ptrs[ap_idx] = ptr;
        }

        for i in 0..n {
            out[i] = audio[i] * (1.0 - wet * 0.5) + self.wet_temp[i] * wet;
        }
    }
}

/// Real-time chorus effect with modulated delay line.
#[derive(Debug, Clone)]
pub struct ChorusEffect {
    sr: usize,
    max_delay: usize,
    buffer: Vec<f32>,
    write_ptr: usize,
    lfo_phase: f32,
}

impl ChorusEffect {
    pub fn new(sample_rate: usize) -> Self {
        let max_delay = (sample_rate as f32 * 0.03) as usize; // 30ms max delay
        Self {
            sr: sample_rate,
            max_delay,
            buffer: vec![0.0; max_delay],
            write_ptr: 0,
            lfo_phase: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_ptr = 0;
        self.lfo_phase = 0.0;
    }

    pub fn process(&mut self, audio: &[f32], depth: f32, rate: f32, out: &mut [f32]) {
        if depth <= 0.01 {
            out.copy_from_slice(audio);
            return;
        }

        let n = audio.len();
        let buf_len = self.max_delay;
        let lfo_step = 2.0 * PI * rate / (self.sr as f32);
        let base_delay = (buf_len as f32) * 0.5;
        let mod_amp = (buf_len as f32) * 0.35 * depth;

        for i in 0..n {
            self.buffer[self.write_ptr] = audio[i];
            let delay = base_delay + mod_amp * self.lfo_phase.sin();
            self.lfo_phase += lfo_step;
            if self.lfo_phase > 2.0 * PI {
                self.lfo_phase -= 2.0 * PI;
            }

            let tap = (self.write_ptr as f32) - delay;
            let tap_floor = tap.floor();
            let idx0 = ((tap_floor as isize).rem_euclid(buf_len as isize)) as usize;
            let idx1 = (idx0 + 1) % buf_len;
            let frac = tap - tap_floor;

            let delayed_val = (1.0 - frac) * self.buffer[idx0] + frac * self.buffer[idx1];
            out[i] = audio[i] * 0.7 + delayed_val * 0.5 * depth;
            self.write_ptr = (self.write_ptr + 1) % buf_len;
        }
    }
}

/// 2nd-order IIR Biquad filter (Direct Form II Transposed).
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl BiquadFilter {
    /// Telephone bandpass (300 Hz – 3400 Hz) matching Python's
    /// `scipy.signal.butter(2, [300, 3400], btype="bandpass")` target
    /// response: flat 0 dB passband with steep skirts.
    ///
    /// Uses the standard RBJ "band pass (constant 0 dB peak gain)" design
    /// with Q = f0 / BW, where f0 is the geometric center and BW the
    /// bandwidth in Hz.
    pub fn new_butterworth_bandpass(sample_rate: f32, f_low: f32, f_high: f32) -> Self {
        let f0 = (f_low * f_high).sqrt();
        let q = f0 / (f_high - f_low).max(1.0);
        let w0 = 2.0 * PI * f0 / sample_rate;
        let alpha = w0.sin() / (2.0 * q.max(0.05));

        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Complete Vocal DSP processing engine.
#[derive(Debug, Clone)]
pub struct VoiceDSP {
    pub sample_rate: usize,
    pub options: DSPOptions,

    pitch_shifter: ContinuousPitchShifter,
    reverb: SchroederReverb,
    chorus: ChorusEffect,
    radio_filter: BiquadFilter,

    robot_carrier_phase: f32,
    noise_gate_envelope: f32,

    scratch_a: Vec<f32>,
    scratch_b: Vec<f32>,
}

impl VoiceDSP {
    pub fn new(sample_rate: usize, block_size: usize) -> Self {
        let radio_filter =
            BiquadFilter::new_butterworth_bandpass(sample_rate as f32, 300.0, 3400.0);

        Self {
            sample_rate,
            options: DSPOptions::default(),

            pitch_shifter: ContinuousPitchShifter::new(sample_rate),
            reverb: SchroederReverb::new(sample_rate),
            chorus: ChorusEffect::new(sample_rate),
            radio_filter,

            robot_carrier_phase: 0.0,
            noise_gate_envelope: -100.0,

            scratch_a: vec![0.0; block_size.max(1024)],
            scratch_b: vec![0.0; block_size.max(1024)],
        }
    }

    pub fn update_options(&mut self, options: DSPOptions) {
        self.options = options;
    }

    pub fn reset(&mut self) {
        self.pitch_shifter.reset();
        self.reverb.reset();
        self.chorus.reset();
        self.radio_filter.reset();
        self.robot_carrier_phase = 0.0;
        self.noise_gate_envelope = -100.0;
    }

    /// Process mono block of audio samples in place or into destination buffer.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let n = input.len();
        if output.len() < n {
            return;
        }

        if self.scratch_a.len() < n {
            self.scratch_a.resize(n, 0.0);
            self.scratch_b.resize(n, 0.0);
        }

        if self.options.bypass {
            let gain = self.options.output_gain;
            for (i, &sample) in input.iter().enumerate().take(n) {
                output[i] = sample * gain;
            }
            return;
        }

        // 1. Input Gain
        let in_gain = self.options.input_gain;
        for (i, &sample) in input.iter().enumerate().take(n) {
            self.scratch_a[i] = sample * in_gain;
        }

        // 2. Noise Gate (Soft Knee)
        if self.options.noise_gate_enabled {
            let mut sum_sq = 0.0;
            for &sample in self.scratch_a.iter().take(n) {
                sum_sq += sample * sample;
            }
            let rms = (sum_sq / (n as f32) + 1e-12).sqrt();
            let db = 20.0 * rms.log10();

            let alpha = if db > self.noise_gate_envelope {
                0.45
            } else {
                0.04
            };
            self.noise_gate_envelope = (1.0 - alpha) * self.noise_gate_envelope + alpha * db;

            let threshold = self.options.noise_gate_threshold_db;
            let knee_width = 8.0;

            if self.noise_gate_envelope < threshold - knee_width {
                self.scratch_a[..n].fill(0.0);
            } else if self.noise_gate_envelope < threshold {
                let gate_ratio =
                    (self.noise_gate_envelope - (threshold - knee_width)) / knee_width;
                for sample in self.scratch_a.iter_mut().take(n) {
                    *sample *= gate_ratio;
                }
            }
        }

        // 3. Pitch Shift
        if self.options.pitch_semitones.abs() >= 0.1 {
            self.pitch_shifter.process(
                &self.scratch_a[..n],
                self.options.pitch_semitones,
                &mut self.scratch_b[..n],
            );
            self.scratch_a[..n].copy_from_slice(&self.scratch_b[..n]);
        }

        // 4. Robot Voice / Ring Modulation
        if self.options.robot_enabled && self.options.robot_freq > 0.0 {
            let step = 2.0 * PI * self.options.robot_freq / (self.sample_rate as f32);
            let mix = self.options.robot_mix;
            for i in 0..n {
                let carrier = (self.robot_carrier_phase + (i as f32) * step).sin();
                let modulated = self.scratch_a[i] * carrier;
                self.scratch_a[i] = self.scratch_a[i] * (1.0 - mix) + modulated * mix * 1.4;
            }
            self.robot_carrier_phase =
                (self.robot_carrier_phase + (n as f32) * step).rem_euclid(2.0 * PI);
        }

        // 5. Radio / Walkie-Talkie Filter
        if self.options.radio_enabled {
            for sample in self.scratch_a.iter_mut().take(n) {
                let filtered = self.radio_filter.process_sample(*sample);
                *sample = (filtered * 2.2).tanh() * 0.75;
            }
        }

        // 6. Distortion / Drive
        if self.options.distortion_enabled && self.options.distortion_drive > 0.01 {
            let drive_factor = 1.0 + self.options.distortion_drive * 8.0;
            let norm = 1.0 / (1.0 + self.options.distortion_drive * 0.5);
            for sample in self.scratch_a.iter_mut().take(n) {
                *sample = (*sample * drive_factor).tanh() * norm;
            }
        }

        // 7. Chorus
        if self.options.chorus_enabled && self.options.chorus_depth > 0.01 {
            self.chorus.process(
                &self.scratch_a[..n],
                self.options.chorus_depth,
                self.options.chorus_rate,
                &mut self.scratch_b[..n],
            );
            self.scratch_a[..n].copy_from_slice(&self.scratch_b[..n]);
        }

        // 8. Reverb
        if self.options.reverb_enabled && self.options.reverb_wet > 0.01 {
            self.reverb.process(
                &self.scratch_a[..n],
                self.options.reverb_room_size,
                self.options.reverb_wet,
                &mut self.scratch_b[..n],
            );
            self.scratch_a[..n].copy_from_slice(&self.scratch_b[..n]);
        }

        // 9. Output Gain & Soft Limiter
        let out_gain = self.options.output_gain;
        for (i, out_sample) in output.iter_mut().enumerate().take(n) {
            let mut val = self.scratch_a[i] * out_gain;
            if val.abs() > 0.95 {
                val = val.tanh();
            }
            *out_sample = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsp_bypass() {
        let mut dsp = VoiceDSP::new(48000, 256);
        dsp.options.bypass = true;
        let input = vec![0.5; 256];
        let mut output = vec![0.0; 256];
        dsp.process(&input, &mut output);
        assert_eq!(input, output);
    }

    #[test]
    fn test_pitch_shifter() {
        let mut dsp = VoiceDSP::new(48000, 256);
        dsp.options.pitch_semitones = 4.0;
        let input = vec![0.2; 256];
        let mut output = vec![0.0; 256];
        dsp.process(&input, &mut output);
        // On subsequent blocks grain synthesis outputs shifted signals
        dsp.process(&input, &mut output);
        assert!(output.iter().any(|&x| x.abs() > 0.001));
        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_reverb_and_chorus() {
        let mut dsp = VoiceDSP::new(48000, 256);
        dsp.options.reverb_enabled = true;
        dsp.options.reverb_wet = 0.5;
        dsp.options.chorus_enabled = true;
        dsp.options.chorus_depth = 0.5;
        let input = vec![0.1; 256];
        let mut output = vec![0.0; 256];
        dsp.process(&input, &mut output);
        assert!(output.iter().all(|&x| x.is_finite()));
    }
}
