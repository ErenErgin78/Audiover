use crate::audio::dsp::{DSPOptions, VoiceDSP};
use crate::audio::stream::{
    is_virtual_device_name, AudioDevicesState, AudioStreamEngine, Meters,
};
use crate::input::hotkeys::{
    default_bindings, is_known_action, normalize_key, HotkeyManager, HotkeyStatus,
};
use crate::log_buffer::{LogBuffer, LogEntry};
use crate::soundboard::manager::{SoundItem, SoundboardManager};
use crate::soundboard::player::{SoundProgress, SoundboardPlayer};
use base64::Engine;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub config_path: PathBuf,
    pub log_buffer: Arc<LogBuffer>,
}

// ─────────────────────────────────────────────────────────────
// Settings persistence (mirrors Python AudioverAPI._save_settings:
// { app: { language }, audio: { ... }, voice_effects: { custom_presets,
// active_preset } }; the soundboard section is owned by SoundboardManager).
// ─────────────────────────────────────────────────────────────

fn read_settings_file(config_path: &Path) -> serde_json::Value {
    fs::read_to_string(config_path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn write_settings_file(config_path: &Path, value: &serde_json::Value) {
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(formatted) = serde_json::to_string_pretty(value) {
        let _ = fs::write(config_path, formatted);
    }
}

fn persist_audio_settings(state: &AppContext) {
    let mut settings = read_settings_file(&state.config_path);
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    let engine = &state.stream_engine;
    settings["audio"] = serde_json::json!({
        "mic_gain": engine.get_mic_gain(),
        "monitor_gain": engine.get_monitor_gain(),
        "hear_myself": engine.hear_myself.load(Ordering::Relaxed),
        "hear_soundboard": engine.hear_soundboard.load(Ordering::Relaxed),
        "block_size": engine.get_block_size(),
        "input_device_name": engine.selected_input_name().and_then(|n| {
            if is_virtual_device_name(&n) {
                return None;
            }
            let list = AudioStreamEngine::list_input_devices();
            list.into_iter().find(|d| {
                let a = d.name.to_lowercase();
                let b = n.trim().to_lowercase();
                a == b || a.contains(b.as_str())
            }).map(|d| d.name).or(Some(n))
        }),
        "monitor_device_name": engine.selected_monitor_name().and_then(|n| {
            if n != "none" && is_virtual_device_name(&n) {
                None
            } else {
                Some(n)
            }
        }),
    });
    write_settings_file(&state.config_path, &settings);
}

fn persist_voice_settings(state: &AppContext) {
    let mut settings = read_settings_file(&state.config_path);
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    let defaults = get_default_presets();
    let presets = state.presets.lock();
    // Only non-built-in presets (and overrides) are persisted as customs,
    // mirroring Python `voice_effects.custom_presets`.
    let mut customs = serde_json::Map::new();
    for (name, cfg) in presets.iter() {
        if defaults.get(name) != Some(cfg) {
            customs.insert(name.clone(), serde_json::to_value(cfg).unwrap_or_default());
        }
    }
    settings["voice_effects"] = serde_json::json!({
        "custom_presets": customs,
        "active_preset": *state.active_preset.lock(),
    });
    write_settings_file(&state.config_path, &settings);
}

fn persist_language(state: &AppContext) {
    let mut settings = read_settings_file(&state.config_path);
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    if settings.get("app").is_none() {
        settings["app"] = serde_json::json!({});
    }
    settings["app"]["language"] = serde_json::json!(*state.language.lock());
    write_settings_file(&state.config_path, &settings);
}

fn persist_hotkeys(state: &AppContext) {
    let mut settings = read_settings_file(&state.config_path);
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    // Only persist known action bindings (ignore legacy direct-key entries).
    let snapshot = state.hotkey_manager.bindings_snapshot();
    let mut map = serde_json::Map::new();
    for (id, key) in snapshot {
        if is_known_action(&id) {
            map.insert(id, serde_json::Value::String(key));
        }
    }
    settings["hotkeys"] = serde_json::Value::Object(map);
    write_settings_file(&state.config_path, &settings);
}

/// Loads persisted `hotkeys: { action_id: "KEY" }` on startup.
/// Unknown ids and empty keys are dropped; missing ids fall back to defaults.
pub fn load_persisted_hotkeys(config_path: &Path) -> HashMap<String, String> {
    let mut out = default_bindings();
    let settings = read_settings_file(config_path);
    if let Some(map) = settings.get("hotkeys").and_then(|v| v.as_object()) {
        for (id, v) in map {
            if !is_known_action(id) {
                continue;
            }
            if let Some(key) = v.as_str() {
                let normalized = normalize_key(key);
                if !normalized.is_empty() {
                    out.insert(id.clone(), normalized);
                }
            }
        }
    }
    out
}

/// Loads persisted `{ custom_presets, active_preset, language }` on startup.
/// Returns `(custom_presets, active_preset, language)`.
pub fn load_persisted_voice_and_app(
    config_path: &Path,
) -> (HashMap<String, PresetConfig>, Option<String>, Option<String>) {
    let settings = read_settings_file(config_path);
    let mut customs = HashMap::new();
    let mut active = None;
    if let Some(voice) = settings.get("voice_effects") {
        if let Some(map) = voice.get("custom_presets").and_then(|v| v.as_object()) {
            for (k, v) in map {
                if let Ok(cfg) = serde_json::from_value::<PresetConfig>(v.clone()) {
                    customs.insert(k.clone(), cfg);
                }
            }
        }
        active = voice
            .get("active_preset")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    let language = settings
        .get("app")
        .and_then(|a| a.get("language"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (customs, active, language)
}

/// Loads the persisted `audio` section on startup.
pub fn load_persisted_audio(config_path: &Path) -> serde_json::Value {
    let settings = read_settings_file(config_path);
    settings.get("audio").cloned().unwrap_or_else(|| serde_json::json!({}))
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
    if lang != "tr" && lang != "en" {
        return serde_json::json!({ "ok": false, "error": "Unsupported language" });
    }
    *state.language.lock() = lang.clone();
    persist_language(&state);
    serde_json::json!({ "ok": true, "language": lang })
}

#[tauri::command]
pub fn set_engine_active(active: bool, state: State<'_, AppContext>) -> serde_json::Value {
    if active {
        // Report the real outcome: start() can fail (e.g. no mic).
        let ok = state.stream_engine.start().is_ok()
            && state.stream_engine.is_running.load(Ordering::SeqCst);
        serde_json::json!({ "ok": ok, "active": ok })
    } else {
        state.stream_engine.stop();
        serde_json::json!({ "ok": true, "active": false })
    }
}

#[tauri::command]
pub fn set_muted(muted: bool, state: State<'_, AppContext>) {
    state.stream_engine.set_muted(muted);
}

#[tauri::command]
pub fn set_hear_myself(enabled: bool, state: State<'_, AppContext>) {
    state.stream_engine.set_hear_myself(enabled);
    persist_audio_settings(&state);
}

#[tauri::command]
pub fn set_hear_soundboard(enabled: bool, state: State<'_, AppContext>) {
    state.stream_engine.set_hear_soundboard(enabled);
    persist_audio_settings(&state);
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
        drop(presets);
        drop(dsp);
        persist_voice_settings(&state);
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
        drop(presets);
        // Python parity: live tweaks are stored back onto the active preset.
        persist_voice_settings(&state);
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
        drop(presets);
        drop(dsp);
        persist_voice_settings(&state);
        let presets = state.presets.lock();
        serde_json::json!({ "ok": true, "presets": *presets, "config": def_cfg })
    } else {
        serde_json::json!({ "ok": false, "error": "Only default presets can be reset" })
    }
}

#[tauri::command]
pub fn create_preset(name: String, config: PresetConfig, state: State<'_, AppContext>) -> serde_json::Value {
    let name = name.trim().to_string();
    if name.is_empty() {
        return serde_json::json!({ "ok": false, "error": "Name cannot be empty" });
    }
    if get_default_presets().contains_key(&name) {
        return serde_json::json!({ "ok": false, "error": "Cannot overwrite built-in preset" });
    }
    let mut presets = state.presets.lock();
    presets.insert(name.clone(), config);
    *state.active_preset.lock() = name.clone();
    drop(presets);
    persist_voice_settings(&state);
    let presets = state.presets.lock();
    serde_json::json!({ "ok": true, "name": name, "presets": *presets })
}

#[tauri::command]
pub fn save_preset(name: String, config: PresetConfig, state: State<'_, AppContext>) -> serde_json::Value {
    let name = name.trim().to_string();
    if name.is_empty() {
        return serde_json::json!({ "ok": false, "error": "Name cannot be empty" });
    }
    let mut presets = state.presets.lock();
    presets.insert(name, config);
    drop(presets);
    persist_voice_settings(&state);
    let presets = state.presets.lock();
    serde_json::json!({ "ok": true, "presets": *presets })
}

#[tauri::command]
pub fn delete_preset(name: String, state: State<'_, AppContext>) -> serde_json::Value {
    if get_default_presets().contains_key(&name) {
        return serde_json::json!({ "ok": false, "error": "Cannot delete built-in preset" });
    }
    let mut presets = state.presets.lock();
    if !presets.contains_key(&name) {
        return serde_json::json!({ "ok": false, "error": "Preset not found" });
    }
    presets.remove(&name);
    let mut active = state.active_preset.lock();
    if *active == name {
        *active = "Clean".to_string();
        if let Some(clean) = presets.get("Clean").cloned().or_else(|| get_default_presets().get("Clean").cloned()) {
            state.dsp.lock().update_options(preset_to_dsp_options(&clean));
        }
    }
    let active_name = active.clone();
    drop(active);
    drop(presets);
    persist_voice_settings(&state);
    let presets = state.presets.lock();
    serde_json::json!({ "ok": true, "presets": *presets, "active": active_name })
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

#[tauri::command(rename_all = "camelCase")]
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

    // Stage into the system temp dir first; SoundboardManager copies it into
    // the library with a unique `{id}_` prefix (Python parity), so concurrent
    // drops of same-named files can never collide.
    let safe_name = Path::new(&filename)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "dropped-audio".to_string());
    let temp_dest = std::env::temp_dir().join(format!(
        "audiover-drop-{}-{}",
        std::process::id(),
        safe_name
    ));
    if let Err(e) = fs::write(&temp_dest, bytes) {
        return serde_json::json!({ "ok": false, "error": format!("Failed to write file: {}", e) });
    }

    let p_str = temp_dest.to_string_lossy().to_string();
    let result = if let Some(sound) = state.soundboard_manager.add_sound_file(&p_str, Some(safe_name.as_str()), true, None, 1.0, false) {
        serde_json::json!({ "ok": true, "sound": sound })
    } else {
        serde_json::json!({ "ok": false, "error": "Could not decode sound" })
    };
    let _ = fs::remove_file(&temp_dest);
    result
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

#[tauri::command(rename_all = "camelCase")]
pub fn update_sound(
    id: String,
    volume: Option<f32>,
    loop_val: Option<bool>,
    hotkey: Option<String>,
    state: State<'_, AppContext>,
) -> serde_json::Value {
    // Python parity: hotkeys are stored normalized (uppercased, trimmed).
    let hotkey = hotkey.map(|h| {
        let clean = h.trim().to_uppercase();
        if clean.is_empty() {
            String::new()
        } else {
            clean
        }
    });
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
    let mut devices = state.stream_engine.get_devices_state();
    // Python parity: hide Audiover's internal virtual devices from the UI.
    devices.inputs.retain(|d| !is_virtual_device_name(&d.name));
    devices.outputs.retain(|d| !is_virtual_device_name(&d.name));
    devices
}

#[tauri::command]
pub fn set_input_device(index: usize, state: State<'_, AppContext>) {
    state.stream_engine.set_input_device(index);
    persist_audio_settings(&state);
}

#[tauri::command]
pub fn set_monitor_device(index: Option<usize>, state: State<'_, AppContext>) {
    state.stream_engine.set_monitor_device(index);
    persist_audio_settings(&state);
}

#[tauri::command]
pub fn set_buffer_size(size: usize, state: State<'_, AppContext>) {
    state.stream_engine.set_buffer_size(size);
    persist_audio_settings(&state);
}

#[tauri::command]
pub fn set_mic_gain(gain: f32, state: State<'_, AppContext>) {
    state.stream_engine.set_mic_gain(gain);
    persist_audio_settings(&state);
}

#[tauri::command]
pub fn set_monitor_gain(gain: f32, state: State<'_, AppContext>) {
    state.stream_engine.set_monitor_gain(gain);
    persist_audio_settings(&state);
}

#[tauri::command]
pub fn get_hotkey_status(state: State<'_, AppContext>) -> HotkeyStatus {
    state.hotkey_manager.get_status()
}

#[tauri::command(rename_all = "camelCase")]
pub fn set_hotkey(action_id: String, key: String, state: State<'_, AppContext>) -> serde_json::Value {
    let action_id = action_id.trim().to_string();
    if !is_known_action(&action_id) {
        return serde_json::json!({ "ok": false, "error": "Unknown hotkey action" });
    }
    let normalized = normalize_key(&key);
    if normalized.is_empty() {
        return serde_json::json!({ "ok": false, "error": "Key cannot be empty" });
    }
    if normalized.len() > 16 {
        return serde_json::json!({ "ok": false, "error": "Invalid key" });
    }
    // Conflict with another global action?
    let snapshot = state.hotkey_manager.bindings_snapshot();
    for (id, bound) in snapshot.iter() {
        if id != &action_id && *bound == normalized {
            return serde_json::json!({
                "ok": false,
                "error": "conflict",
                "conflict": id,
                "message": format!("Key is already assigned to {}", id),
            });
        }
    }
    // Conflict with a soundboard hotkey?
    for sound in state.soundboard_manager.get_all_sounds() {
        if let Some(hk) = sound.hotkey {
            if hk.trim().to_uppercase() == normalized {
                return serde_json::json!({
                    "ok": false,
                    "error": "conflict",
                    "conflict": sound.name,
                    "message": format!("Key is already used by sound \"{}\"", sound.name),
                });
            }
        }
    }
    // `set_binding` re-checks action conflicts; None means success here.
    if let Some(conflict) = state.hotkey_manager.set_binding(&action_id, &normalized) {
        return serde_json::json!({
            "ok": false,
            "error": "conflict",
            "conflict": conflict,
        });
    }
    persist_hotkeys(&state);
    serde_json::json!({
        "ok": true,
        "actionId": action_id,
        "key": normalized,
        "hotkeys": state.hotkey_manager.get_status().hotkeys,
    })
}

#[tauri::command]
pub fn reset_hotkeys(state: State<'_, AppContext>) -> serde_json::Value {
    state.hotkey_manager.reset_bindings();
    persist_hotkeys(&state);
    serde_json::json!({
        "ok": true,
        "hotkeys": state.hotkey_manager.get_status().hotkeys,
    })
}

#[tauri::command]
pub fn trigger_hotkey(key: String, state: State<'_, AppContext>) -> serde_json::Value {
    let ok = state.hotkey_manager.trigger(&key);
    serde_json::json!({ "ok": ok })
}

// ─────────────────────────────────────────────────────────────
// Logs & diagnostics (in-app log viewer on the Logs page)
// ─────────────────────────────────────────────────────────────

#[tauri::command(rename_all = "camelCase")]
pub fn get_logs(since_seq: Option<u64>, state: State<'_, AppContext>) -> Vec<LogEntry> {
    state.log_buffer.tail_since(since_seq)
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppContext>) {
    state.log_buffer.clear();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostics {
    pub app_version: String,
    pub engine_active: bool,
    pub is_muted: bool,
    pub hear_myself: bool,
    pub hear_soundboard: bool,
    pub mic_gain: f32,
    pub monitor_gain: f32,
    pub block_size: usize,
    pub sample_rate: u32,
    pub effective_sample_rate: u32,
    pub selected_input: Option<String>,
    pub selected_monitor: Option<String>,
    pub input_count: usize,
    pub output_count: usize,
    pub current_input: Option<usize>,
    pub current_monitor: Option<usize>,
    pub virtual_sink_found: bool,
    pub pactl_available: bool,
    pub active_preset: String,
    pub preset_count: usize,
    pub language: String,
    pub hotkey_backend: String,
    pub hotkey_permission: bool,
    pub log_entries: usize,
    pub config_path: String,
}

#[tauri::command]
pub fn get_diagnostics(state: State<'_, AppContext>) -> Diagnostics {
    let engine = &state.stream_engine;
    let devices = engine.get_devices_state();
    let hk = state.hotkey_manager.get_status();
    let pactl_available = std::process::Command::new("pactl")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    Diagnostics {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        engine_active: engine.is_running.load(Ordering::SeqCst),
        is_muted: engine.is_muted.load(Ordering::SeqCst),
        hear_myself: engine.hear_myself.load(Ordering::Relaxed),
        hear_soundboard: engine.hear_soundboard.load(Ordering::Relaxed),
        mic_gain: engine.get_mic_gain(),
        monitor_gain: engine.get_monitor_gain(),
        block_size: engine.get_block_size(),
        sample_rate: engine.sample_rate,
        effective_sample_rate: engine.get_effective_rate(),
        selected_input: engine.selected_input_name(),
        selected_monitor: engine.selected_monitor_name(),
        input_count: devices.inputs.len(),
        output_count: devices.outputs.len(),
        current_input: devices.current_input,
        current_monitor: devices.current_monitor,
        virtual_sink_found: AudioStreamEngine::is_virtual_sink_available(),
        pactl_available,
        active_preset: state.active_preset.lock().clone(),
        preset_count: state.presets.lock().len(),
        language: state.language.lock().clone(),
        hotkey_backend: hk.backend,
        hotkey_permission: hk.has_permission,
        log_entries: state.log_buffer.len(),
        config_path: state.config_path.to_string_lossy().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context(config_path: PathBuf) -> AppContext {
        let dsp = Arc::new(Mutex::new(VoiceDSP::new(48000, 256)));
        let player = Arc::new(SoundboardPlayer::new(48000));
        let sounds_dir = config_path.parent().unwrap().join("sounds");
        let manager = Arc::new(SoundboardManager::new(
            config_path.clone(),
            sounds_dir,
            player.clone(),
        ));
        let engine = Arc::new(AudioStreamEngine::new(48000, 256, dsp.clone(), player.clone()));
        AppContext {
            stream_engine: engine,
            soundboard_player: player,
            soundboard_manager: manager,
            hotkey_manager: Arc::new(HotkeyManager::new()),
            dsp,
            active_preset: Mutex::new("Clean".to_string()),
            presets: Mutex::new(get_default_presets()),
            language: Mutex::new("tr".to_string()),
            config_path,
            log_buffer: Arc::new(LogBuffer::new()),
        }
    }

    fn temp_config_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "audiover-test-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::create_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn audio_settings_round_trip() {
        let path = temp_config_path("audio");
        let _ = fs::remove_file(&path);
        let ctx = test_context(path.clone());
        ctx.stream_engine.set_mic_gain(1.5);
        ctx.stream_engine.set_monitor_gain(0.5);
        ctx.stream_engine.set_hear_myself(true);
        ctx.stream_engine.set_buffer_size(512);
        persist_audio_settings(&ctx);

        let loaded = load_persisted_audio(&path);
        assert_eq!(loaded["mic_gain"].as_f64().unwrap(), 1.5);
        assert_eq!(loaded["monitor_gain"].as_f64().unwrap(), 0.5);
        assert_eq!(loaded["hear_myself"].as_bool().unwrap(), true);
        assert_eq!(loaded["block_size"].as_u64().unwrap(), 512);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn voice_settings_round_trip_with_custom_preset() {
        let path = temp_config_path("voice");
        let _ = fs::remove_file(&path);
        let ctx = test_context(path.clone());
        let mut custom = get_default_presets()["Clean"].clone();
        custom.pitch = -7.0;
        ctx.presets.lock().insert("MyVoice".to_string(), custom);
        *ctx.active_preset.lock() = "MyVoice".to_string();
        *ctx.language.lock() = "en".to_string();
        persist_voice_settings(&ctx);
        persist_language(&ctx);

        let (customs, active, language) = load_persisted_voice_and_app(&path);
        assert_eq!(customs["MyVoice"].pitch, -7.0);
        assert!(!customs.contains_key("Clean"));
        assert_eq!(active.as_deref(), Some("MyVoice"));
        assert_eq!(language.as_deref(), Some("en"));
        let _ = fs::remove_file(&path);
    }
}
