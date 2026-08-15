import math
from dataclasses import dataclass, field
from typing import Optional
import numpy as np
import scipy.signal


@dataclass
class DSPOptions:
    """Configurable parameters for the real-time DSP pipeline."""

    # Master Bypass & Noise Gate
    bypass: bool = False
    noise_gate_enabled: bool = False
    noise_gate_threshold_db: float = -65.0  # dB (gentle threshold)

    # Pitch Shift
    pitch_semitones: float = 0.0  # -12 to +12 semitones

    # Robot / Ring Modulation
    robot_enabled: bool = False
    robot_freq: float = 150.0  # Hz (50Hz - 600Hz)
    robot_mix: float = 0.75

    # Radio / Walkie-Talkie
    radio_enabled: bool = False

    # Distortion / Bitcrush
    distortion_enabled: bool = False
    distortion_drive: float = 0.0  # 0.0 to 1.0

    # Reverb
    reverb_enabled: bool = False
    reverb_room_size: float = 0.0  # 0.0 to 1.0
    reverb_wet: float = 0.0  # 0.0 to 1.0

    # Chorus
    chorus_enabled: bool = False
    chorus_depth: float = 0.0  # 0.0 to 1.0
    chorus_rate: float = 1.2  # Hz

    # Filters
    highpass_cutoff: float = 20.0  # Hz
    lowpass_cutoff: float = 20000.0  # Hz

    # Master Gains
    input_gain: float = 1.0
    output_gain: float = 1.0


class ContinuousPitchShifter:
    """High-performance real-time vocal pitch shifter using vectorized 4-grain synthesis."""

    def __init__(self, sample_rate: int = 48000):
        self.sr = sample_rate
        self.buf_len = int(sample_rate * 0.25)  # 250ms circular buffer
        self.buffer = np.zeros(self.buf_len, dtype=np.float32)
        self.write_ptr = 0
        self.phases = np.array([0.0, 0.25, 0.5, 0.75], dtype=np.float32)

    def reset(self):
        self.buffer.fill(0)
        self.write_ptr = 0
        self.phases = np.array([0.0, 0.25, 0.5, 0.75], dtype=np.float32)

    def process(self, audio: np.ndarray, semitones: float) -> np.ndarray:
        if abs(semitones) < 0.05:
            return audio

        pitch_ratio = 2.0 ** (semitones / 12.0)
        rate = 1.0 - pitch_ratio

        # Adaptive grain size: larger window for deep voice to preserve pitch fundamentals,
        # smaller window for high voice for crisp definition without delay smear.
        if semitones < 0:
            window_ms = 35.0 + min(abs(semitones) * 1.5, 25.0)  # 35-60ms
        else:
            window_ms = max(20.0 - semitones * 0.6, 12.0)  # 12-20ms

        w_size = int(self.sr * (window_ms / 1000.0))
        n = len(audio)
        buf_len = self.buf_len

        # Store incoming block into circular buffer
        write_indices = (self.write_ptr + np.arange(n)) % buf_len
        self.buffer[write_indices] = audio
        curr_write_ptrs = self.write_ptr + np.arange(n)

        # Vectorized 4-grain synthesis with Hann window
        steps = np.arange(n, dtype=np.float32) * (rate / w_size)
        out = np.zeros(n, dtype=np.float32)

        for k in range(4):
            phase_k = (self.phases[k] + steps) % 1.0
            # Raised cosine window
            w = 0.5 * (1.0 - np.cos(2.0 * np.pi * phase_k))
            delay_samples = phase_k * w_size
            tap = curr_write_ptrs - delay_samples

            idx0 = np.floor(tap).astype(np.int64) % buf_len
            idx1 = (idx0 + 1) % buf_len
            frac = tap - np.floor(tap)

            grain_val = (1.0 - frac) * self.buffer[idx0] + frac * self.buffer[idx1]
            out += (w * grain_val)

        # 4 overlapping Hann windows sum to 2.0 -> normalize by 0.5
        out *= 0.5

        # Update phases
        self.phases = (self.phases + n * (rate / w_size)) % 1.0
        self.write_ptr = (self.write_ptr + n) % buf_len
        return out.astype(np.float32)


class SchroederReverb:
    """Low-latency Schroeder Reverb with 4 comb filters and 2 all-pass diffusers."""

    def __init__(self, sample_rate: int = 48000):
        self.sr = sample_rate
        # Delay lengths in samples scaled for 48kHz
        comb_delays = [
            int(sample_rate * t)
            for t in [0.0297, 0.0371, 0.0411, 0.0437]  # ~30ms to ~44ms
        ]
        allpass_delays = [
            int(sample_rate * t) for t in [0.0050, 0.0017]  # ~5ms, ~1.7ms
        ]

        self.comb_buffers = [np.zeros(d, dtype=np.float32) for d in comb_delays]
        self.comb_ptrs = [0] * len(comb_delays)

        self.ap_buffers = [np.zeros(d, dtype=np.float32) for d in allpass_delays]
        self.ap_ptrs = [0] * len(allpass_delays)

    def reset(self):
        for b in self.comb_buffers:
            b.fill(0)
        for b in self.ap_buffers:
            b.fill(0)
        self.comb_ptrs = [0] * len(self.comb_buffers)
        self.ap_ptrs = [0] * len(self.ap_buffers)

    def process(
        self, audio: np.ndarray, room_size: float, wet: float
    ) -> np.ndarray:
        if wet <= 0.001:
            return audio

        feedback = 0.65 + 0.28 * min(max(room_size, 0.0), 1.0)
        out_wet = np.zeros_like(audio)
        n = len(audio)

        # 4 Parallel Comb Filters
        for c_idx, buf in enumerate(self.comb_buffers):
            b_len = len(buf)
            ptr = self.comb_ptrs[c_idx]
            for i in range(n):
                delayed = buf[ptr]
                buf[ptr] = audio[i] + delayed * feedback
                out_wet[i] += delayed * 0.25
                ptr = (ptr + 1) % b_len
            self.comb_ptrs[c_idx] = ptr

        # 2 Series All-Pass Filters
        ap_gain = 0.5
        for ap_idx, buf in enumerate(self.ap_buffers):
            b_len = len(buf)
            ptr = self.ap_ptrs[ap_idx]
            for i in range(n):
                delayed = buf[ptr]
                in_val = out_wet[i]
                buf[ptr] = in_val + delayed * ap_gain
                out_wet[i] = delayed - in_val * ap_gain
                ptr = (ptr + 1) % b_len
            self.ap_ptrs[ap_idx] = ptr

        return audio * (1.0 - wet * 0.5) + out_wet * wet


class ChorusEffect:
    """Real-time stereo/mono chorus effect with modulated delay line."""

    def __init__(self, sample_rate: int = 48000):
        self.sr = sample_rate
        self.max_delay = int(sample_rate * 0.03)  # 30ms max delay
        self.buffer = np.zeros(self.max_delay, dtype=np.float32)
        self.write_ptr = 0
        self.lfo_phase = 0.0

    def reset(self):
        self.buffer.fill(0)
        self.write_ptr = 0
        self.lfo_phase = 0.0

    def process(
        self, audio: np.ndarray, depth: float, rate: float = 1.2
    ) -> np.ndarray:
        if depth <= 0.01:
            return audio

        n = len(audio)
        out = np.empty_like(audio)
        buf_len = self.max_delay
        lfo_step = 2.0 * math.pi * rate / self.sr
        base_delay = buf_len * 0.5
        mod_amp = buf_len * 0.35 * depth

        for i in range(n):
            self.buffer[self.write_ptr] = audio[i]
            delay = base_delay + mod_amp * math.sin(self.lfo_phase)
            self.lfo_phase += lfo_step
            if self.lfo_phase > 2.0 * math.pi:
                self.lfo_phase -= 2.0 * math.pi

            tap = self.write_ptr - delay
            idx0 = int(math.floor(tap)) % buf_len
            idx1 = (idx0 + 1) % buf_len
            frac = tap - math.floor(tap)
            delayed_val = (
                1.0 - frac
            ) * self.buffer[idx0] + frac * self.buffer[idx1]

            out[i] = audio[i] * 0.7 + delayed_val * 0.5 * depth
            self.write_ptr = (self.write_ptr + 1) % buf_len

        return out


class VoiceDSP:
    """Main Real-Time DSP Audio Processing Pipeline."""

    def __init__(self, sample_rate: int = 48000, block_size: int = 256):
        self.sr = sample_rate
        self.block_size = block_size
        self.options = DSPOptions()

        # DSP Submodules
        self.pitch_shifter = ContinuousPitchShifter(sample_rate)
        self.reverb = SchroederReverb(sample_rate)
        self.chorus = ChorusEffect(sample_rate)

        # Carrier state for Ring Modulator / Robot voice
        self.robot_carrier_phase = 0.0

        # Radio bandpass filter design (300Hz - 3400Hz)
        self._init_filters()

        # Noise Gate Envelope follower state (-100 dB represents baseline silence)
        self.noise_gate_envelope = -100.0

    def _init_filters(self):
        nyquist = self.sr * 0.5
        # Radio bandpass filter
        low = 300.0 / nyquist
        high = 3400.0 / nyquist
        self.radio_b, self.radio_a = scipy.signal.butter(
            2, [low, high], btype="bandpass"
        )
        self.radio_zi = scipy.signal.lfilter_zi(self.radio_b, self.radio_a) * 0

    def update_options(self, options: DSPOptions):
        self.options = options

    def reset(self):
        """Resets all filter and delay states."""
        self.pitch_shifter.reset()
        self.reverb.reset()
        self.chorus.reset()
        self.robot_carrier_phase = 0.0
        self.radio_zi = np.zeros_like(self.radio_zi)
        self.noise_gate_envelope = -100.0

    def process(self, input_block: np.ndarray) -> np.ndarray:
        """Processes a single 1D or 2D audio block in real-time.

        Args:
            input_block: float32 NumPy array with values normalized to [-1.0, 1.0].
        Returns:
            processed audio block with the same shape.
        """
        if self.options.bypass:
            return input_block * self.options.output_gain

        # Handle stereo input by converting or averaging for mono processing
        is_stereo = (input_block.ndim == 2 and input_block.shape[1] == 2) or (
            input_block.ndim == 2 and input_block.shape[0] == 2
        )

        if input_block.ndim == 2:
            if input_block.shape[1] == 2:
                mono = 0.5 * (input_block[:, 0] + input_block[:, 1])
            else:
                mono = 0.5 * (input_block[0, :] + input_block[1, :])
        else:
            mono = input_block.copy()

        # 1. Input Gain
        if self.options.input_gain != 1.0:
            mono = mono * self.options.input_gain

        # 2. Noise Gate (Pre-DSP) with Soft Knee
        if self.options.noise_gate_enabled:
            rms = np.sqrt(np.mean(mono**2) + 1e-12)
            db = 20.0 * np.log10(rms)
            # Fast attack (0.45) to open instantly on speech, smooth release (0.04)
            alpha = 0.45 if db > self.noise_gate_envelope else 0.04
            self.noise_gate_envelope = (
                1.0 - alpha
            ) * self.noise_gate_envelope + alpha * db

            threshold = self.options.noise_gate_threshold_db
            knee_width = 8.0  # dB
            if self.noise_gate_envelope < threshold - knee_width:
                mono *= 0.0
            elif self.noise_gate_envelope < threshold:
                gate_ratio = (
                    self.noise_gate_envelope - (threshold - knee_width)
                ) / knee_width
                mono *= float(gate_ratio)

        # 3. Pitch Shift
        if abs(self.options.pitch_semitones) >= 0.1:
            mono = self.pitch_shifter.process(
                mono, self.options.pitch_semitones
            )

        # 4. Robot Voice / Ring Modulation
        if self.options.robot_enabled and self.options.robot_freq > 0:
            n_samples = len(mono)
            step = 2.0 * math.pi * self.options.robot_freq / self.sr
            t = self.robot_carrier_phase + np.arange(n_samples) * step
            carrier = np.sin(t).astype(np.float32)
            self.robot_carrier_phase = (
                self.robot_carrier_phase + n_samples * step
            ) % (2.0 * math.pi)

            # Modulate and blend
            modulated = mono * carrier
            mix = self.options.robot_mix
            mono = mono * (1.0 - mix) + modulated * mix * 1.4

        # 5. Radio / Walkie-Talkie Filter
        if self.options.radio_enabled:
            filtered, self.radio_zi = scipy.signal.lfilter(
                self.radio_b, self.radio_a, mono, zi=self.radio_zi
            )
            # Add slight overdrive/resonance
            mono = np.tanh(filtered * 2.2) * 0.75

        # 6. Distortion / Drive
        if (
            self.options.distortion_enabled
            and self.options.distortion_drive > 0.01
        ):
            drive_factor = 1.0 + self.options.distortion_drive * 8.0
            mono = np.tanh(mono * drive_factor) * (
                1.0 / (1.0 + self.options.distortion_drive * 0.5)
            )

        # 7. Chorus
        if self.options.chorus_enabled and self.options.chorus_depth > 0.01:
            mono = self.chorus.process(
                mono, self.options.chorus_depth, self.options.chorus_rate
            )

        # 8. Reverb
        if self.options.reverb_enabled and self.options.reverb_wet > 0.01:
            mono = self.reverb.process(
                mono, self.options.reverb_room_size, self.options.reverb_wet
            )

        # 9. Output Gain & Soft Knee Limiter
        mono = mono * self.options.output_gain
        # Soft limiter using tanh to prevent harsh clipping
        over = np.abs(mono) > 0.95
        if np.any(over):
            mono = np.tanh(mono)

        # Reshape to original input format
        if input_block.ndim == 2:
            if input_block.shape[1] == 2:
                return np.column_stack((mono, mono)).astype(np.float32)
            else:
                return np.row_stack((mono, mono)).astype(np.float32)
        return mono.astype(np.float32)
