use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
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
}

pub struct SoundboardPlayer {
    target_sample_rate: u32,
    tracks: parking_lot::RwLock<HashMap<String, SoundTrack>>,
}

impl SoundboardPlayer {
    pub fn new(target_sample_rate: u32) -> Self {
        Self {
            target_sample_rate,
            tracks: parking_lot::RwLock::new(HashMap::new()),
        }
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

        let (mut raw_stereo, sr) = match decode_audio_file(path) {
            Ok(res) => res,
            Err(e) => {
                error!("Failed to decode audio file {}: {}", file_path, e);
                return None;
            }
        };

        // Resample if needed
        if sr != self.target_sample_rate && !raw_stereo.is_empty() {
            raw_stereo = resample_stereo(&raw_stereo, sr, self.target_sample_rate);
        }

        let duration_sec = if self.target_sample_rate > 0 {
            raw_stereo.len() as f32 / self.target_sample_rate as f32
        } else {
            0.0
        };

        let track = SoundTrack {
            name: sound_name,
            audio_data: raw_stereo,
            duration_sec,
            position: 0,
            volume: volume.clamp(0.0, 2.0),
            loop_playback,
            is_playing: false,
        };

        self.tracks.write().insert(sound_id.to_string(), track.clone());
        info!("Loaded sound '{}' ({:.2}s)", track.name, track.duration_sec);
        Some(track)
    }

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
        if let Some(track) = self.tracks.write().get_mut(sound_id) {
            track.is_playing = !track.is_playing;
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

/// Resample stereo audio from in_sr to out_sr
fn resample_stereo(input: &[[f32; 2]], in_sr: u32, out_sr: u32) -> Vec<[f32; 2]> {
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
