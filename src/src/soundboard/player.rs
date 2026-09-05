use log::{error, info, warn};
use rubato::{
    audioadapter::Adapter,
    audioadapter_buffers::direct::InterleavedSlice,
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundProgress {
    pub is_playing: bool,
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub struct SoundTrack {
    pub name: String,
    pub audio_data: Vec<[f32; 2]>, // interleaved stereo frames
    pub duration_sec: f32,
    pub position: usize,
    pub volume: f32,
    pub loop_playback: bool,
    pub is_playing: bool,
    pub raw_audio_data: Vec<[f32; 2]>,
    pub raw_sample_rate: u32,
}

pub struct SoundboardPlayer {
    target_sample_rate: AtomicU32,
    tracks: parking_lot::RwLock<HashMap<String, SoundTrack>>,
}

impl SoundboardPlayer {
    pub fn new(target_sample_rate: u32) -> Self {
        Self {
            target_sample_rate: AtomicU32::new(target_sample_rate.max(4000)),
            tracks: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    pub fn target_sample_rate(&self) -> u32 {
        self.target_sample_rate.load(Ordering::Relaxed)
    }

    /// Retargets already-loaded tracks to the sample rate actually
    /// negotiated with the audio device. Without this, tracks decoded at
    /// 48 kHz but consumed at e.g. 16 kHz play back 3x too fast with
    /// dropouts — the classic "robotic soundboard" symptom. Same-rate
    /// calls are a no-op so repeated engine restarts never degrade quality
    /// through cumulative resampling.
    pub fn set_target_sample_rate(&self, sample_rate: u32) {
        let new_rate = sample_rate.clamp(4000, 192000);
        let old_rate = self.target_sample_rate.swap(new_rate, Ordering::SeqCst);
        if old_rate == new_rate {
            return;
        }
        let mut tracks = self.tracks.write();
        for track in tracks.values_mut() {
            if track.raw_audio_data.is_empty() && track.audio_data.is_empty() {
                track.duration_sec = 0.0;
                track.position = 0;
                continue;
            }
            // Use pristine decoded original to eliminate cumulative generational loss
            let (source_data, source_rate) = if !track.raw_audio_data.is_empty() && track.raw_sample_rate > 0 {
                (&track.raw_audio_data, track.raw_sample_rate)
            } else {
                (&track.audio_data, old_rate)
            };

            track.audio_data = resample_stereo(source_data, source_rate, new_rate);
            track.duration_sec = track.audio_data.len() as f32 / new_rate as f32;
            track.position = track.position.min(track.audio_data.len());
        }
        info!(
            "Soundboard retargeted {} tracks: {} Hz -> {} Hz",
            tracks.len(),
            old_rate,
            new_rate
        );
    }

    pub fn load_sound(
        &self,
        sound_id: &str,
        file_path: &str,
        name: Option<&str>,
        volume: f32,
        loop_playback: bool,
    ) -> Option<SoundTrack> {
        let path = Path::new(file_path);
        if !path.exists() {
            error!("Sound file not found: {}", file_path);
            return None;
        }

        let sound_name = name
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());

        let (raw_stereo, sr) = match decode_audio_file(path) {
            Ok(res) => res,
            Err(e) => {
                error!("Failed to decode audio file {}: {}", file_path, e);
                return None;
            }
        };

        // Resample if needed
        let target_rate = self.target_sample_rate();
        let audio_data = if sr != target_rate && !raw_stereo.is_empty() {
            resample_stereo(&raw_stereo, sr, target_rate)
        } else {
            raw_stereo.clone()
        };

        let duration_sec = if target_rate > 0 {
            audio_data.len() as f32 / target_rate as f32
        } else {
            0.0
        };

        let track = SoundTrack {
            name: sound_name,
            audio_data,
            duration_sec,
            position: 0,
            volume: volume.clamp(0.0, 2.0),
            loop_playback,
            is_playing: false,
            raw_audio_data: raw_stereo,
            raw_sample_rate: sr,
        };

        self.tracks.write().insert(sound_id.to_string(), track.clone());
        info!("Loaded sound '{}' ({:.2}s)", track.name, track.duration_sec);
        Some(track)
    }

    #[allow(dead_code)]
    pub fn is_playing(&self, sound_id: &str) -> bool {
        self.tracks
            .read()
            .get(sound_id)
            .map(|t| t.is_playing)
            .unwrap_or(false)
    }

    pub fn play(&self, sound_id: &str) {
        if let Some(track) = self.tracks.write().get_mut(sound_id) {
            track.position = 0;
            track.is_playing = true;
        }
    }

    pub fn pause(&self, sound_id: &str) {
        // Python parity: pause halts playback without resetting position
        // (it never toggles back to playing).
        if let Some(track) = self.tracks.write().get_mut(sound_id) {
            track.is_playing = false;
        }
    }

    pub fn stop(&self, sound_id: &str) {
        if let Some(track) = self.tracks.write().get_mut(sound_id) {
            track.position = 0;
            track.is_playing = false;
        }
    }

    pub fn stop_all(&self) {
        let mut tracks = self.tracks.write();
        for track in tracks.values_mut() {
            track.position = 0;
            track.is_playing = false;
        }
    }

    pub fn update_track(&self, sound_id: &str, volume: Option<f32>, loop_playback: Option<bool>) {
        if let Some(track) = self.tracks.write().get_mut(sound_id) {
            if let Some(v) = volume {
                track.volume = v.clamp(0.0, 2.0);
            }
            if let Some(lp) = loop_playback {
                track.loop_playback = lp;
            }
        }
    }

    pub fn remove_track(&self, sound_id: &str) {
        self.tracks.write().remove(sound_id);
    }

    pub fn get_all_progress(&self) -> HashMap<String, SoundProgress> {
        let tracks = self.tracks.read();
        let mut map = HashMap::new();
        for (id, track) in tracks.iter() {
            let total = track.audio_data.len();
            let progress = if total > 0 {
                (track.position as f32) / (total as f32)
            } else {
                0.0
            };
            map.insert(
                id.clone(),
                SoundProgress {
                    is_playing: track.is_playing,
                    progress,
                },
            );
        }
        map
    }

    /// Mix playing tracks into the provided stereo buffer (left, right).
    pub fn mix_into(&self, output_stereo: &mut [[f32; 2]], master_gain: f32) {
        if master_gain <= 0.0001 {
            return;
        }

        let mut tracks = self.tracks.write();
        let num_frames = output_stereo.len();

        for track in tracks.values_mut() {
            if !track.is_playing || track.audio_data.is_empty() {
                continue;
            }

            let track_gain = track.volume * master_gain;
            let total_samples = track.audio_data.len();

            for frame in output_stereo.iter_mut().take(num_frames) {
                if track.position >= total_samples {
                    if track.loop_playback {
                        track.position = 0;
                    } else {
                        track.is_playing = false;
                        track.position = 0;
                        break;
                    }
                }

                let s = track.audio_data[track.position];
                frame[0] += s[0] * track_gain;
                frame[1] += s[1] * track_gain;
                track.position += 1;
            }
        }
    }
}

/// Decode audio file using Symphonia 0.6.1 into a vector of stereo samples [L, R]
fn decode_audio_file(path: &Path) -> Result<(Vec<[f32; 2]>, u32), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| format!("Probe failed: {}", e))?;

    let (track_id, audio_params) = format
        .tracks()
        .iter()
        .find_map(|t| {
            t.codec_params
                .as_ref()
                .and_then(|cp| cp.audio())
                .map(|ap| (t.id, ap.clone()))
        })
        .ok_or_else(|| "No supported audio tracks found".to_string())?;

    let sample_rate = audio_params
        .sample_rate
        .ok_or_else(|| "Unknown sample rate".to_string())?;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("Cannot create decoder: {}", e))?;

    let mut stereo_samples: Vec<[f32; 2]> = Vec::new();
    let mut interleaved_f32: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => {
                warn!("Decode packet error: {}", e);
                break;
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let channels = decoded.spec().channels().count();
                interleaved_f32.clear();
                decoded.copy_to_vec_interleaved(&mut interleaved_f32);

                if channels == 1 {
                    for &s in &interleaved_f32 {
                        stereo_samples.push([s, s]);
                    }
                } else if channels >= 2 {
                    let num_frames = interleaved_f32.len() / channels;
                    for i in 0..num_frames {
                        let l = interleaved_f32[i * channels];
                        let r = interleaved_f32[i * channels + 1];
                        stereo_samples.push([l, r]);
                    }
                }
            }
            Err(SymphoniaError::DecodeError(e)) => {
                warn!("Decode error ignored: {}", e);
            }
            Err(e) => {
                warn!("Fatal decode error: {}", e);
                break;
            }
        }
    }

    Ok((stereo_samples, sample_rate))
}

/// Resample stereo audio from in_sr to out_sr using band-limited sinc interpolation (rubato)
pub fn resample_stereo(input: &[[f32; 2]], in_sr: u32, out_sr: u32) -> Vec<[f32; 2]> {
    if input.is_empty() || in_sr == out_sr {
        return input.to_vec();
    }

    let in_sr_f = in_sr.max(4000) as f64;
    let out_sr_f = out_sr.max(4000) as f64;
    let ratio = out_sr_f / in_sr_f;

    // Use Blackman-Harris windowed sinc filter with automatic anti-aliasing cutoff calculation.
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: None,
        oversampling_factor: 128,
        interpolation: SincInterpolationType::Cubic,
        window: WindowFunction::BlackmanHarris2,
    };

    let flat_input: &[f32] = unsafe {
        std::slice::from_raw_parts(input.as_ptr() as *const f32, input.len() * 2)
    };

    let input_adapter = match InterleavedSlice::new(flat_input, 2, input.len()) {
        Ok(a) => a,
        Err(_) => return fallback_resample_stereo(input, in_sr, out_sr),
    };

    let chunk_size = input.len().clamp(64, 1024);
    let mut resampler = match Async::<f32>::new_sinc(
        ratio,
        1.1,
        &params,
        chunk_size,
        2,
        FixedAsync::Input,
    ) {
        Ok(r) => r,
        Err(e) => {
            warn!(
                "Failed to initialize sinc resampler ({} Hz -> {} Hz): {}, using fallback",
                in_sr, out_sr, e
            );
            return fallback_resample_stereo(input, in_sr, out_sr);
        }
    };

    match resampler.process_all(&input_adapter, input.len(), None) {
        Ok(output) => {
            let frames = output.frames();
            let data = output.take_data();
            let mut result = Vec::with_capacity(frames);
            for chunk in data.chunks_exact(2) {
                result.push([chunk[0], chunk[1]]);
            }
            result
        }
        Err(e) => {
            warn!(
                "Sinc resampling failed ({} Hz -> {} Hz): {}, using fallback",
                in_sr, out_sr, e
            );
            fallback_resample_stereo(input, in_sr, out_sr)
        }
    }
}

/// Fallback resampler using cubic interpolation if sinc initialization fails
fn fallback_resample_stereo(input: &[[f32; 2]], in_sr: u32, out_sr: u32) -> Vec<[f32; 2]> {
    if input.is_empty() || in_sr == out_sr {
        return input.to_vec();
    }

    let ratio = out_sr as f64 / in_sr as f64;
    let target_len = ((input.len() as f64) * ratio).round() as usize;
    let mut output = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let src_idx = (i as f64) / ratio;
        let idx0 = (src_idx.floor() as usize).min(input.len() - 1);
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let frac = (src_idx - src_idx.floor()) as f32;

        let l = input[idx0][0] * (1.0 - frac) + input[idx1][0] * frac;
        let r = input[idx0][1] * (1.0 - frac) + input[idx1][1] * frac;
        output.push([l, r]);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_test_track(player: &SoundboardPlayer, id: &str, frames: usize) {
        let data = vec![[0.5, 0.5]; frames];
        let sr = player.target_sample_rate();
        player.tracks.write().insert(
            id.to_string(),
            SoundTrack {
                name: id.to_string(),
                audio_data: data.clone(),
                duration_sec: frames as f32 / sr as f32,
                position: 0,
                volume: 1.0,
                loop_playback: false,
                is_playing: false,
                raw_audio_data: data,
                raw_sample_rate: sr,
            },
        );
    }

    #[test]
    fn pause_halts_without_toggling() {
        let player = SoundboardPlayer::new(48000);
        insert_test_track(&player, "s1", 1024);
        player.play("s1");
        assert!(player.is_playing("s1"));
        player.pause("s1");
        assert!(!player.is_playing("s1"));
        // A second pause must NOT resume playback (Python parity).
        player.pause("s1");
        assert!(!player.is_playing("s1"));
    }

    #[test]
    fn play_restarts_from_zero() {
        let player = SoundboardPlayer::new(48000);
        insert_test_track(&player, "s1", 1024);
        player.play("s1");
        let mut buf = vec![[0.0, 0.0]; 256];
        player.mix_into(&mut buf, 1.0);
        assert_eq!(player.tracks.read()["s1"].position, 256);
        player.play("s1");
        assert_eq!(player.tracks.read()["s1"].position, 0);
    }

    #[test]
    fn mix_advances_position_exactly_once_per_block() {
        let player = SoundboardPlayer::new(48000);
        insert_test_track(&player, "s1", 4096);
        player.play("s1");
        let mut buf = vec![[0.0, 0.0]; 256];
        player.mix_into(&mut buf, 1.0);
        // One mix call consumes exactly one block; the engine now mixes once
        // per input block and shares the result (no double-speed playback).
        assert_eq!(player.tracks.read()["s1"].position, 256);
        assert!(buf.iter().all(|f| f[0] == 0.5 && f[1] == 0.5));
    }

    #[test]
    fn retarget_rescales_tracks_and_stays_idempotent() {
        let player = SoundboardPlayer::new(48000);
        insert_test_track(&player, "s1", 4800); // 0.1 s @ 48 kHz
        player.play("s1");
        player.set_target_sample_rate(16000);
        assert_eq!(player.target_sample_rate(), 16000);
        // 4800 frames @ 48 kHz -> ~1600 frames @ 16 kHz, same duration.
        {
            let tracks = player.tracks.read();
            let track = &tracks["s1"];
            assert!((track.audio_data.len() as i32 - 1600).abs() <= 2);
            assert!((track.duration_sec - 0.1).abs() < 0.002);
        }
        // Repeated restarts must not resample again (no quality decay).
        let snapshot = player.tracks.read()["s1"].audio_data.clone();
        player.set_target_sample_rate(16000);
        assert_eq!(player.tracks.read()["s1"].audio_data, snapshot);
        // Back to 48 kHz restores the original length.
        player.set_target_sample_rate(48000);
        assert!((player.tracks.read()["s1"].audio_data.len() as i32 - 4800).abs() <= 2);
    }

    #[test]
    fn test_sinc_resample_accuracy() {
        // Generate a 440 Hz test tone at 44.1 kHz
        let sr_in = 44100;
        let sr_out = 48000;
        let n_frames = 4410; // 0.1 sec
        let mut original = Vec::with_capacity(n_frames);
        for i in 0..n_frames {
            let t = i as f32 / sr_in as f32;
            let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            original.push([val, val]);
        }

        let resampled = resample_stereo(&original, sr_in, sr_out);
        let expected_frames = (n_frames as f64 * sr_out as f64 / sr_in as f64).round() as usize;
        assert!((resampled.len() as i32 - expected_frames as i32).abs() <= 2);
    }
}
