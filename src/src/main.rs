// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod input;
mod soundboard;

use audio::dsp::VoiceDSP;
use audio::router::AudioRouter;
use audio::stream::AudioStreamEngine;
use commands::{get_default_presets, preset_to_dsp_options, AppContext};
use input::hotkeys::HotkeyManager;
use log::info;
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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting Audiover Rust/Tauri Engine...");

    let sample_rate = 48000;
    let block_size = 256;

    // 1. Virtual Audio Router
    let router = AudioRouter::new(
        "Audiover_Sink",
        "Audiover_Virtual_Sink",
        "Audiover_Mic",
        "Audiover_Virtual_Microphone",
    );
    router.setup_virtual_devices();

    // 2. DSP Engine
    let dsp = Arc::new(Mutex::new(VoiceDSP::new(sample_rate as usize, block_size)));

    // 3. Soundboard Player & Manager
    let (config_path, sounds_dir) = resolve_paths();
    let soundboard_player = Arc::new(SoundboardPlayer::new(sample_rate));
    let soundboard_manager = Arc::new(SoundboardManager::new(
        config_path,
        sounds_dir,
        soundboard_player.clone(),
    ));
    soundboard_manager.load_from_config();

    // 4. Stream Engine
    let stream_engine = Arc::new(AudioStreamEngine::new(
        sample_rate,
        block_size,
        dsp.clone(),
        soundboard_player.clone(),
    ));
    let _ = stream_engine.start();

    // 5. Hotkey Manager
    let hotkey_manager = Arc::new(HotkeyManager::new());

    // Register default global hotkeys
    {
        let stream_m = stream_engine.clone();
        hotkey_manager.register("F9", move || {
            let current = stream_m.is_muted.load(std::sync::atomic::Ordering::Relaxed);
            stream_m.set_muted(!current);
        });
    }
    {
        let dsp_b = dsp.clone();
        hotkey_manager.register("F10", move || {
            let mut d = dsp_b.lock();
            let mut opts = d.options.clone();
            opts.bypass = !opts.bypass;
            d.update_options(opts);
        });
    }
    {
        let sb_p = soundboard_player.clone();
        hotkey_manager.register("F11", move || {
            sb_p.stop_all();
        });
    }
    {
        let stream_h = stream_engine.clone();
        hotkey_manager.register("F8", move || {
            let current = stream_h.hear_myself.load(std::sync::atomic::Ordering::Relaxed);
            stream_h.set_hear_myself(!current);
        });
    }

    {
        let sb_m = soundboard_manager.clone();
        hotkey_manager.set_fallback(move |key| {
            sb_m.play_by_hotkey(key)
        });
    }

    hotkey_manager.start();

    // 6. Default Presets & Context
    let default_presets = get_default_presets();
    let initial_preset = "Clean".to_string();
    if let Some(cfg) = default_presets.get(&initial_preset) {
        dsp.lock().update_options(preset_to_dsp_options(cfg));
    }

    let context = AppContext {
        stream_engine: stream_engine.clone(),
        soundboard_player: soundboard_player.clone(),
        soundboard_manager: soundboard_manager.clone(),
        hotkey_manager: hotkey_manager.clone(),
        dsp: dsp.clone(),
        active_preset: Mutex::new(initial_preset),
        presets: Mutex::new(default_presets),
        language: Mutex::new("tr".to_string()),
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
            commands::trigger_hotkey,
        ])
        .run(tauri::generate_context!())
        .expect("error while running audiover application");

    // Cleanup on exit
    info!("Shutting down Audiover...");
    stream_engine.stop();
    hotkey_manager.stop();
    router.cleanup();
}
