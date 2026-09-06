// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod input;
mod log_buffer;
mod soundboard;

use audio::dsp::VoiceDSP;
use audio::router::AudioRouter;
use audio::stream::AudioStreamEngine;
use commands::{
    get_default_presets, load_persisted_audio, load_persisted_hotkeys,
    load_persisted_voice_and_app, preset_to_dsp_options, AppContext,
};
use input::hotkeys::HotkeyManager;
use log::{error, info, warn};
use log_buffer::{install_logger, LogBuffer};
use parking_lot::Mutex;
use soundboard::manager::SoundboardManager;
use soundboard::player::SoundboardPlayer;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn resolve_paths() -> (PathBuf, PathBuf) {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("audiover");
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("audiover")
        .join("sounds");

    let _ = fs::create_dir_all(&config_dir);
    let _ = fs::create_dir_all(&data_dir);

    let config_path = config_dir.join("settings.json");

    // Seed default settings on first run (Python parity: main.py copies the
    // bundled config when the user config is absent).
    if !config_path.exists() {
        let config_candidates = [
            PathBuf::from("config/settings.json"),
            PathBuf::from("../config/settings.json"),
            PathBuf::from("/opt/audiover/config/settings.json"),
            PathBuf::from("/usr/share/audiover/config/settings.json"),
        ];
        for bundled in &config_candidates {
            if bundled.exists() {
                let _ = fs::copy(bundled, &config_path);
                break;
            }
        }
    }

    // Seed default sample sounds from candidate locations
    let candidates = [
        PathBuf::from("assets/sounds"),
        PathBuf::from("../assets/sounds"),
        PathBuf::from("/opt/audiover/assets/sounds"),
        PathBuf::from("/usr/share/audiover/assets/sounds"),
    ];
    for bundled in &candidates {
        if bundled.exists() {
            if let Ok(entries) = fs::read_dir(bundled) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let dest = data_dir.join(p.file_name().unwrap_or_default());
                        if !dest.exists() {
                            let _ = fs::copy(&p, dest);
                        }
                    }
                }
            }
            break;
        }
    }

    (config_path, data_dir)
}

fn main() {
    // In-memory ring buffer feeding the in-app log viewer; the combined
    // logger also keeps writing to stderr, so terminal output is unchanged.
    let log_buffer = Arc::new(LogBuffer::new());
    install_logger(log_buffer.clone());

    info!(
        "Starting Audiover v{} Rust/Tauri Engine...",
        env!("CARGO_PKG_VERSION")
    );

    let sample_rate = 48000;

    // 1. Virtual Audio Router
    let router = AudioRouter::new(
        "Audiover_Sink",
        "Audiover_Virtual_Sink",
        "Audiover_Mic",
        "Audiover_Virtual_Microphone",
    );
    if !AudioRouter::is_pipewire_available() {
        warn!("PipeWire/PulseAudio daemon not reachable via pactl; virtual devices unavailable.");
    }
    if !router.setup_virtual_devices() {
        warn!("Virtual audio devices could not be created; virtual-mic output will be silent.");
    }

    // 2. DSP Engine (block size restored from persisted audio settings,
    //    mirroring Python main.py).
    let (config_path, sounds_dir) = resolve_paths();
    let saved_audio = load_persisted_audio(&config_path);
    let block_size = saved_audio
        .get("block_size")
        .and_then(|v| v.as_u64())
        .map(|v| (v as usize).clamp(64, 4096))
        .unwrap_or(256);
    let dsp = Arc::new(Mutex::new(VoiceDSP::new(sample_rate as usize, block_size)));

    // 3. Soundboard Player & Manager
    let soundboard_player = Arc::new(SoundboardPlayer::new(sample_rate));
    let soundboard_manager = Arc::new(SoundboardManager::new(
        config_path.clone(),
        sounds_dir,
        soundboard_player.clone(),
    ));
    soundboard_manager.load_from_config();

    // 4. Stream Engine + persisted audio state (gains, hear flags, devices).
    let stream_engine = Arc::new(AudioStreamEngine::new(
        sample_rate,
        block_size,
        dsp.clone(),
        soundboard_player.clone(),
    ));
    if let Some(g) = saved_audio.get("mic_gain").and_then(|v| v.as_f64()) {
        stream_engine.set_mic_gain(g as f32);
    }
    if let Some(g) = saved_audio.get("monitor_gain").and_then(|v| v.as_f64()) {
        stream_engine.set_monitor_gain(g as f32);
    }
    if let Some(h) = saved_audio.get("hear_myself").and_then(|v| v.as_bool()) {
        // Set the flag directly: the monitor stream is (re)opened by start().
        stream_engine.hear_myself.store(h, std::sync::atomic::Ordering::SeqCst);
    }
    if let Some(h) = saved_audio.get("hear_soundboard").and_then(|v| v.as_bool()) {
        stream_engine.hear_soundboard.store(h, std::sync::atomic::Ordering::SeqCst);
    }
    if let Some(name) = saved_audio.get("input_device_name").and_then(|v| v.as_str()) {
        stream_engine.set_input_device_name(name);
    }
    if let Some(name) = saved_audio.get("monitor_device_name").and_then(|v| v.as_str()) {
        stream_engine.set_monitor_device_name(name);
    }
    if let Err(e) = stream_engine.start() {
        error!("Audio engine failed to start: {}", e);
    }
    info!(
        "Audio config: block_size={} mic_gain={} monitor_gain={} hear_myself={} hear_soundboard={} config={}",
        stream_engine.get_block_size(),
        stream_engine.get_mic_gain(),
        stream_engine.get_monitor_gain(),
        stream_engine.hear_myself.load(std::sync::atomic::Ordering::Relaxed),
        stream_engine.hear_soundboard.load(std::sync::atomic::Ordering::Relaxed),
        config_path.display(),
    );

    // 5. Hotkey Manager (bindings restored from settings.json `hotkeys`).
    let persisted_bindings = load_persisted_hotkeys(&config_path);
    let hotkey_manager = Arc::new(HotkeyManager::with_bindings(persisted_bindings.clone()));
    let key_for = |id: &str, fallback: &str| -> String {
        persisted_bindings
            .get(id)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    };

    // Register global hotkeys under stable action ids so they can be remapped.
    {
        let stream_m = stream_engine.clone();
        hotkey_manager.register_action("mute_mic", &key_for("mute_mic", "F9"), move || {
            let current = stream_m.is_muted.load(std::sync::atomic::Ordering::Relaxed);
            stream_m.set_muted(!current);
        });
    }
    {
        let dsp_b = dsp.clone();
        hotkey_manager.register_action("bypass_dsp", &key_for("bypass_dsp", "F10"), move || {
            let mut d = dsp_b.lock();
            let mut opts = d.options.clone();
            opts.bypass = !opts.bypass;
            d.update_options(opts);
        });
    }
    {
        let sb_p = soundboard_player.clone();
        hotkey_manager.register_action("stop_all", &key_for("stop_all", "F11"), move || {
            sb_p.stop_all();
        });
    }
    {
        let stream_h = stream_engine.clone();
        hotkey_manager.register_action(
            "toggle_hear_myself",
            &key_for("toggle_hear_myself", "F8"),
            move || {
                let current = stream_h.hear_myself.load(std::sync::atomic::Ordering::Relaxed);
                stream_h.set_hear_myself(!current);
            },
        );
    }

    {
        let sb_m = soundboard_manager.clone();
        hotkey_manager.set_fallback(move |key| {
            sb_m.play_by_hotkey(key)
        });
    }

    hotkey_manager.start();

    // 6. Presets, active preset & language (restored from settings.json,
    //    mirroring Python AudioverAPI._load_all_settings).
    let (custom_presets, saved_active, saved_language) =
        load_persisted_voice_and_app(&config_path);
    let mut all_presets = get_default_presets();
    for (name, cfg) in custom_presets {
        all_presets.insert(name, cfg);
    }
    let initial_preset = saved_active
        .filter(|name| all_presets.contains_key(name))
        .unwrap_or_else(|| "Clean".to_string());
    if let Some(cfg) = all_presets.get(&initial_preset) {
        dsp.lock().update_options(preset_to_dsp_options(cfg));
    }
    let initial_language = saved_language
        .filter(|l| l == "tr" || l == "en")
        .unwrap_or_else(|| "tr".to_string());

    let context = AppContext {
        stream_engine: stream_engine.clone(),
        soundboard_player: soundboard_player.clone(),
        soundboard_manager: soundboard_manager.clone(),
        hotkey_manager: hotkey_manager.clone(),
        dsp: dsp.clone(),
        active_preset: Mutex::new(initial_preset),
        presets: Mutex::new(all_presets),
        language: Mutex::new(initial_language),
        config_path: config_path.clone(),
        log_buffer: log_buffer.clone(),
    };

    tauri::Builder::default()
        .manage(context)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_language,
            commands::set_engine_active,
            commands::set_muted,
            commands::set_hear_myself,
            commands::set_hear_soundboard,
            commands::get_meters,
            commands::get_presets,
            commands::apply_preset,
            commands::update_dsp,
            commands::reset_preset,
            commands::create_preset,
            commands::save_preset,
            commands::delete_preset,
            commands::get_sounds,
            commands::add_sound_file,
            commands::add_sound_data,
            commands::play_sound,
            commands::pause_sound,
            commands::stop_sound,
            commands::stop_all_sounds,
            commands::get_all_progress,
            commands::update_sound,
            commands::remove_sound,
            commands::get_audio_devices,
            commands::set_input_device,
            commands::set_monitor_device,
            commands::set_buffer_size,
            commands::set_mic_gain,
            commands::set_monitor_gain,
            commands::get_hotkey_status,
            commands::set_hotkey,
            commands::reset_hotkeys,
            commands::trigger_hotkey,
            commands::get_logs,
            commands::clear_logs,
            commands::get_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running audiover application");

    // Cleanup on exit
    info!("Shutting down Audiover...");
    stream_engine.stop();
    hotkey_manager.stop();
    router.cleanup();
}
