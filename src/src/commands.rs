use crate::audio::dsp::{DSPOptions, VoiceDSP};
use crate::audio::stream::{AudioDevicesState, AudioStreamEngine, Meters};
use crate::input::hotkeys::{HotkeyManager, HotkeyStatus};
use crate::soundboard::manager::{SoundItem, SoundboardManager};
use crate::soundboard::player::{SoundProgress, SoundboardPlayer};
use base64::Engine;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetConfig {
    pub pitch: f32,
    pub robot: bool,
    pub rfreq: f32,
    pub rmix: f32,
    pub radio: bool,
    pub dist: bool,
    pub drive: f32,
    pub rev: bool,
    pub rsize: f32,
    pub rwet: f32,
    pub chorus: bool,
    pub cdepth: f32,
    #[serde(default)]
    pub bypass: Option<bool>,
    #[serde(default)]
    pub gate: Option<bool>,
    #[serde(default = "default_gate_db")]
    pub gate_db: Option<f32>,
}

fn default_gate_db() -> Option<f32> {
    Some(-65.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatePayload {
    pub engine_active: bool,
    pub is_muted: bool,
    pub hear_myself: bool,
    pub hear_soundboard: bool,
    pub mic_gain: f32,
    pub monitor_gain: f32,
    pub active_preset: String,
    pub presets: HashMap<String, PresetConfig>,
    pub hotkey_permission: bool,
    pub hotkey_backend: String,
    pub language: String,
}

pub struct AppContext {
    pub stream_engine: Arc<AudioStreamEngine>,
    pub soundboard_player: Arc<SoundboardPlayer>,
    pub soundboard_manager: Arc<SoundboardManager>,
    pub hotkey_manager: Arc<HotkeyManager>,
    pub dsp: Arc<Mutex<VoiceDSP>>,
    pub active_preset: Mutex<String>,
    pub presets: Mutex<HashMap<String, PresetConfig>>,
    pub language: Mutex<String>,
}

pub fn get_default_presets() -> HashMap<String, PresetConfig> {
    let mut map = HashMap::new();
    map.insert(
        "Clean".to_string(),
        PresetConfig {
            pitch: 0.0,
            robot: false,
            rfreq: 150.0,
            rmix: 0.0,
            radio: false,
            dist: false,
            drive: 0.0,
            rev: false,
            rsize: 0.0,
            rwet: 0.0,
            chorus: false,
            cdepth: 0.0,
            bypass: Some(false),
            gate: Some(false),
            gate_db: Some(-65.0),
        },
    );
    map.insert(
        "Deep Voice".to_string(),
        PresetConfig {
            pitch: -5.5,
            robot: false,
            rfreq: 120.0,
            rmix: 0.0,
            radio: false,
            dist: true,
            drive: 0.20,
            rev: true,
            rsize: 0.40,
            rwet: 0.25,
            chorus: false,
            cdepth: 0.0,
            bypass: Some(false),
            gate: Some(false),
            gate_db: Some(-65.0),
        },
    );
    map.insert(
        "Radio / Walkie-Talkie".to_string(),
        PresetConfig {
            pitch: 0.0,
            robot: false,
            rfreq: 150.0,
            rmix: 0.0,
            radio: true,
            dist: true,
            drive: 0.35,
            rev: false,
            rsize: 0.0,
            rwet: 0.0,
            chorus: false,
            cdepth: 0.0,
            bypass: Some(false),
            gate: Some(false),
            gate_db: Some(-65.0),
        },
    );
    map.insert(
        "Robot".to_string(),
        PresetConfig {
            pitch: 0.0,
            robot: true,
            rfreq: 140.0,
            rmix: 0.85,
            radio: false,
            dist: false,
            drive: 0.0,
            rev: true,
            rsize: 0.3,
            rwet: 0.2,
            chorus: false,
            cdepth: 0.0,
            bypass: Some(false),
            gate: Some(false),
            gate_db: Some(-65.0),
        },
    );
    map.insert(
        "Alien".to_string(),
        PresetConfig {
            pitch: 3.5,
            robot: true,
            rfreq: 320.0,
            rmix: 0.6,
            radio: false,
            dist: false,
            drive: 0.0,
            rev: true,
            rsize: 0.6,
            rwet: 0.4,
            chorus: true,
            cdepth: 0.5,
            bypass: Some(false),
            gate: Some(false),
            gate_db: Some(-65.0),
        },
    );
    map.insert(
        "Chipmunk".to_string(),
        PresetConfig {
            pitch: 8.5,
            robot: false,
            rfreq: 150.0,
            rmix: 0.0,
            radio: false,
            dist: false,
            drive: 0.0,
            rev: false,
            rsize: 0.0,
            rwet: 0.0,
            chorus: false,
            cdepth: 0.0,
            bypass: Some(false),
            gate: Some(false),
            gate_db: Some(-65.0),
        },
    );
    map
}

pub fn preset_to_dsp_options(p: &PresetConfig) -> DSPOptions {
    DSPOptions {
        bypass: p.bypass.unwrap_or(false),
        noise_gate_enabled: p.gate.unwrap_or(false),
        noise_gate_threshold_db: p.gate_db.unwrap_or(-65.0),
        pitch_semitones: p.pitch,
        robot_enabled: p.robot,
        robot_freq: p.rfreq,
        robot_mix: p.rmix,
        radio_enabled: p.radio,
        distortion_enabled: p.dist,
        distortion_drive: p.drive,
        reverb_enabled: p.rev,
        reverb_room_size: p.rsize,
        reverb_wet: p.rwet,
        chorus_enabled: p.chorus,
        chorus_depth: p.cdepth,
        chorus_rate: 1.2,
        highpass_cutoff: 20.0,
        lowpass_cutoff: 20000.0,
        input_gain: 1.0,
        output_gain: 1.0,
    }
}

// ─────────────────────────────────────────────────────────────
// TAURI COMMANDS
// ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_state(state: State<'_, AppContext>) -> AppStatePayload {
    let hk_status = state.hotkey_manager.get_status();
    AppStatePayload {
        engine_active: state.stream_engine.is_running.load(Ordering::Relaxed),
        is_muted: state.stream_engine.is_muted.load(Ordering::Relaxed),
        hear_myself: state.stream_engine.hear_myself.load(Ordering::Relaxed),
        hear_soundboard: state.stream_engine.hear_soundboard.load(Ordering::Relaxed),
        mic_gain: state.stream_engine.get_mic_gain(),
        monitor_gain: state.stream_engine.get_monitor_gain(),
        active_preset: state.active_preset.lock().clone(),
        presets: state.presets.lock().clone(),
        hotkey_permission: hk_status.has_permission,
        hotkey_backend: hk_status.backend,
        language: state.language.lock().clone(),
    }
}

#[tauri::command]
pub fn set_language(lang: String, state: State<'_, AppContext>) -> serde_json::Value {
    *state.language.lock() = lang.clone();
    serde_json::json!({ "ok": true, "language": lang })
}

#[tauri::command]
pub fn set_engine_active(active: bool, state: State<'_, AppContext>) -> serde_json::Value {
    if active {
        let _ = state.stream_engine.start();
    } else {
        state.stream_engine.stop();
    }
    serde_json::json!({ "ok": true, "active": active })
}

#[tauri::command]
pub fn set_muted(muted: bool, state: State<'_, AppContext>) {
    state.stream_engine.set_muted(muted);
}

#[tauri::command]
pub fn set_hear_myself(enabled: bool, state: State<'_, AppContext>) {
    state.stream_engine.set_hear_myself(enabled);
}

#[tauri::command]
pub fn set_hear_soundboard(enabled: bool, state: State<'_, AppContext>) {
    state.stream_engine.set_hear_soundboard(enabled);
}

#[tauri::command]
pub fn get_meters(state: State<'_, AppContext>) -> Meters {
    state.stream_engine.get_meters()
}

#[tauri::command]
pub fn get_presets(state: State<'_, AppContext>) -> serde_json::Value {
    serde_json::json!({
        "presets": *state.presets.lock(),
        "active": *state.active_preset.lock()
    })
}

#[tauri::command]
pub fn apply_preset(name: String, state: State<'_, AppContext>) -> serde_json::Value {
    let presets = state.presets.lock();
    if let Some(cfg) = presets.get(&name) {
        let dsp_opts = preset_to_dsp_options(cfg);
        let mut dsp = state.dsp.lock();
        dsp.reset();
        dsp.update_options(dsp_opts);
        *state.active_preset.lock() = name.clone();
        serde_json::json!({ "ok": true, "active": name })
    } else {
        serde_json::json!({ "ok": false, "error": "Preset not found" })
    }
}

#[tauri::command]
pub fn update_dsp(opts: serde_json::Value, state: State<'_, AppContext>) {
    let mut presets = state.presets.lock();
    let active = state.active_preset.lock().clone();
    if let Some(cfg) = presets.get_mut(&active) {
        if let Some(p) = opts.get("pitch").and_then(|v| v.as_f64()) { cfg.pitch = p as f32; }
        if let Some(r) = opts.get("robot").and_then(|v| v.as_bool()) { cfg.robot = r; }
        if let Some(rf) = opts.get("rfreq").and_then(|v| v.as_f64()) { cfg.rfreq = rf as f32; }
        if let Some(rm) = opts.get("rmix").and_then(|v| v.as_f64()) { cfg.rmix = rm as f32; }
        if let Some(rd) = opts.get("radio").and_then(|v| v.as_bool()) { cfg.radio = rd; }
        if let Some(d) = opts.get("dist").and_then(|v| v.as_bool()) { cfg.dist = d; }
        if let Some(dr) = opts.get("drive").and_then(|v| v.as_f64()) { cfg.drive = dr as f32; }
        if let Some(rv) = opts.get("rev").and_then(|v| v.as_bool()) { cfg.rev = rv; }
        if let Some(rs) = opts.get("rsize").and_then(|v| v.as_f64()) { cfg.rsize = rs as f32; }
        if let Some(rw) = opts.get("rwet").and_then(|v| v.as_f64()) { cfg.rwet = rw as f32; }
        if let Some(c) = opts.get("chorus").and_then(|v| v.as_bool()) { cfg.chorus = c; }
        if let Some(cd) = opts.get("cdepth").and_then(|v| v.as_f64()) { cfg.cdepth = cd as f32; }
        if let Some(bp) = opts.get("bypass").and_then(|v| v.as_bool()) { cfg.bypass = Some(bp); }
        if let Some(g) = opts.get("gate").and_then(|v| v.as_bool()) { cfg.gate = Some(g); }
        if let Some(gdb) = opts.get("gate_db").and_then(|v| v.as_f64()) { cfg.gate_db = Some(gdb as f32); }

        let dsp_opts = preset_to_dsp_options(cfg);
        state.dsp.lock().update_options(dsp_opts);
    }
}

#[tauri::command]
pub fn reset_preset(name: String, state: State<'_, AppContext>) -> serde_json::Value {
    let defaults = get_default_presets();
    if let Some(def_cfg) = defaults.get(&name) {
        let mut presets = state.presets.lock();
        presets.insert(name.clone(), def_cfg.clone());
        let dsp_opts = preset_to_dsp_options(def_cfg);
        let mut dsp = state.dsp.lock();
        dsp.reset();
        dsp.update_options(dsp_opts);
        serde_json::json!({ "ok": true, "presets": *presets, "config": def_cfg })
    } else {
        serde_json::json!({ "ok": false, "error": "Preset cannot be reset" })
    }
}

#[tauri::command]
pub fn create_preset(name: String, config: PresetConfig, state: State<'_, AppContext>) -> serde_json::Value {
    let mut presets = state.presets.lock();
    presets.insert(name.clone(), config);
    *state.active_preset.lock() = name.clone();
    serde_json::json!({ "ok": true, "name": name, "presets": *presets })
}

#[tauri::command]
pub fn save_preset(name: String, config: PresetConfig, state: State<'_, AppContext>) -> serde_json::Value {
    let mut presets = state.presets.lock();
    presets.insert(name, config);
    serde_json::json!({ "ok": true, "presets": *presets })
}

#[tauri::command]
pub fn delete_preset(name: String, state: State<'_, AppContext>) -> serde_json::Value {
    let mut presets = state.presets.lock();
    presets.remove(&name);
    let mut active = state.active_preset.lock();
    if *active == name {
        *active = "Clean".to_string();
    }
    serde_json::json!({ "ok": true, "presets": *presets, "active": *active })
}

#[tauri::command]
pub fn get_sounds(state: State<'_, AppContext>) -> Vec<SoundItem> {
    state.soundboard_manager.get_all_sounds()
}

#[tauri::command]
pub fn add_sound_file(state: State<'_, AppContext>) -> serde_json::Value {
    let file_opt = rfd::FileDialog::new()
        .add_filter("Audio Files", &["mp3", "wav", "ogg", "flac", "m4a", "aac", "mp4"])
        .pick_file();

    if let Some(path) = file_opt {
        let path_str = path.to_string_lossy().to_string();
        if let Some(sound) = state.soundboard_manager.add_sound_file(&path_str, None, true, None, 1.0, false) {
            serde_json::json!({ "ok": true, "sound": sound })
        } else {
            serde_json::json!({ "ok": false, "error": "Failed to load sound file" })
        }
    } else {
        serde_json::json!({ "ok": false, "cancelled": true })
    }
}

#[tauri::command]
pub fn add_sound_data(filename: String, base64_data: String, state: State<'_, AppContext>) -> serde_json::Value {
    let clean_b64 = if let Some(idx) = base64_data.find(",") {
        &base64_data[idx + 1..]
    } else {
        &base64_data
    };

    let bytes = match base64::engine::general_purpose::STANDARD.decode(clean_b64) {
        Ok(b) => b,
        Err(e) => return serde_json::json!({ "ok": false, "error": format!("Invalid base64: {}", e) }),
    };

    let temp_dest = state.soundboard_manager.sounds_dir.join(&filename);
    if let Err(e) = fs::write(&temp_dest, bytes) {
        return serde_json::json!({ "ok": false, "error": format!("Failed to write file: {}", e) });
    }

    let p_str = temp_dest.to_string_lossy().to_string();
    if let Some(sound) = state.soundboard_manager.add_sound_file(&p_str, None, false, None, 1.0, false) {
        serde_json::json!({ "ok": true, "sound": sound })
    } else {
        serde_json::json!({ "ok": false, "error": "Could not decode sound" })
    }
}

#[tauri::command]
pub fn play_sound(id: String, state: State<'_, AppContext>) {
    state.soundboard_player.play(&id);
}

#[tauri::command]
pub fn pause_sound(id: String, state: State<'_, AppContext>) {
    state.soundboard_player.pause(&id);
}

#[tauri::command]
pub fn stop_sound(id: String, state: State<'_, AppContext>) {
    state.soundboard_player.stop(&id);
}

#[tauri::command]
pub fn stop_all_sounds(state: State<'_, AppContext>) {
    state.soundboard_player.stop_all();
}

#[tauri::command]
pub fn get_all_progress(state: State<'_, AppContext>) -> HashMap<String, SoundProgress> {
    state.soundboard_player.get_all_progress()
}

#[tauri::command]
pub fn update_sound(
    id: String,
    volume: Option<f32>,
    loop_val: Option<bool>,
    hotkey: Option<String>,
    state: State<'_, AppContext>,
) -> serde_json::Value {
    let ok = state.soundboard_manager.update_sound(&id, volume, loop_val, hotkey);
    serde_json::json!({ "ok": ok })
}

#[tauri::command]
pub fn remove_sound(id: String, state: State<'_, AppContext>) -> serde_json::Value {
    let ok = state.soundboard_manager.remove_sound(&id);
    serde_json::json!({ "ok": ok })
}

#[tauri::command]
pub fn get_audio_devices(state: State<'_, AppContext>) -> AudioDevicesState {
    state.stream_engine.get_devices_state()
}

#[tauri::command]
pub fn set_input_device(index: usize, state: State<'_, AppContext>) {
    state.stream_engine.set_input_device(index);
}

#[tauri::command]
pub fn set_monitor_device(index: Option<usize>, state: State<'_, AppContext>) {
    state.stream_engine.set_monitor_device(index);
}

#[tauri::command]
pub fn set_buffer_size(size: usize, state: State<'_, AppContext>) {
    state.stream_engine.set_buffer_size(size);
}

#[tauri::command]
pub fn set_mic_gain(gain: f32, state: State<'_, AppContext>) {
    state.stream_engine.set_mic_gain(gain);
}

#[tauri::command]
pub fn set_monitor_gain(gain: f32, state: State<'_, AppContext>) {
    state.stream_engine.set_monitor_gain(gain);
}

#[tauri::command]
pub fn get_hotkey_status(state: State<'_, AppContext>) -> HotkeyStatus {
    state.hotkey_manager.get_status()
}

#[tauri::command]
pub fn trigger_hotkey(key: String, state: State<'_, AppContext>) -> serde_json::Value {
    let ok = state.hotkey_manager.trigger(&key);
    serde_json::json!({ "ok": ok })
}
