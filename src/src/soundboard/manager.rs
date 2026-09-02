use super::player::SoundboardPlayer;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundItem {
    pub id: String,
    pub name: String,
    pub file_path: String,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub loop_playback: bool,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub duration_sec: f32,
}

fn default_volume() -> f32 {
    1.0
}

fn default_category() -> String {
    "General".to_string()
}

pub struct SoundboardManager {
    pub config_path: PathBuf,
    pub sounds_dir: PathBuf,
    pub player: Arc<SoundboardPlayer>,
    pub sounds: parking_lot::RwLock<HashMap<String, SoundItem>>,
}

impl SoundboardManager {
    pub fn new(
        config_path: PathBuf,
        sounds_dir: PathBuf,
        player: Arc<SoundboardPlayer>,
    ) -> Self {
        let _ = fs::create_dir_all(&sounds_dir);
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        Self {
            config_path,
            sounds_dir,
            player,
            sounds: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    pub fn load_from_config(&self) {
        if !self.config_path.exists() {
            return;
        }

        let content = match fs::read_to_string(&self.config_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Could not read config file: {}", e);
                return;
            }
        };

        let json_val: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse config json: {}", e);
                return;
            }
        };

        let sounds_arr = json_val
            .get("soundboard")
            .and_then(|sb| sb.get("sounds"))
            .and_then(|s| s.as_array());

        if let Some(sound_list) = sounds_arr {
            let mut map = self.sounds.write();
            for item_val in sound_list {
                let id = item_val
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string()[..8].to_string());

                let mut file_path = item_val
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                if !Path::new(&file_path).exists() {
                    let alt = self.sounds_dir.join(
                        Path::new(&file_path)
                            .file_name()
                            .unwrap_or_default(),
                    );
                    if alt.exists() {
                        file_path = alt.to_string_lossy().to_string();
                    } else {
                        warn!("Sound file not found: {}, skipping...", file_path);
                        continue;
                    }
                }

                let name = item_val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        Path::new(&file_path)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });

                let hotkey = item_val
                    .get("hotkey")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let volume = item_val
                    .get("volume")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(1.0);

                let loop_playback = item_val
                    .get("loop")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let category = item_val
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("General")
                    .to_string();

                let mut duration_sec = item_val
                    .get("duration_sec")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(0.0);

                if let Some(track) = self.player.load_sound(
                    &id,
                    &file_path,
                    Some(&name),
                    volume,
                    loop_playback,
                ) {
                    duration_sec = track.duration_sec;
                }

                let sound_item = SoundItem {
                    id: id.clone(),
                    name,
                    file_path,
                    hotkey,
                    volume,
                    loop_playback,
                    category,
                    duration_sec,
                };

                map.insert(id, sound_item);
            }
            info!("Loaded {} sounds into soundboard manager.", map.len());
        }
    }

    pub fn save_to_config(&self) {
        let mut settings: serde_json::Value = if self.config_path.exists() {
            fs::read_to_string(&self.config_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let sounds_map = self.sounds.read();
        let sound_items_vec: Vec<serde_json::Value> = sounds_map
            .values()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "name": item.name,
                    "file_path": item.file_path,
                    "hotkey": item.hotkey,
                    "volume": item.volume,
                    "loop": item.loop_playback,
                    "category": item.category,
                    "duration_sec": item.duration_sec
                })
            })
            .collect();

        if !settings.is_object() {
            settings = serde_json::json!({});
        }

        if settings.get("soundboard").is_none() {
            settings["soundboard"] = serde_json::json!({});
        }
        settings["soundboard"]["sounds"] = serde_json::json!(sound_items_vec);

        if let Ok(formatted) = serde_json::to_string_pretty(&settings) {
            let _ = fs::write(&self.config_path, formatted);
            info!("Saved soundboard settings to {:?}", self.config_path);
        }
    }

    pub fn add_sound_file(
        &self,
        file_path: &str,
        name: Option<&str>,
        copy_to_assets: bool,
        hotkey: Option<String>,
        volume: f32,
        loop_playback: bool,
    ) -> Option<SoundItem> {
        let path = Path::new(file_path);
        if !path.exists() {
            return None;
        }

        let sound_id = Uuid::new_v4().to_string()[..8].to_string();
        let final_name = name
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());

        let target_path = if copy_to_assets {
            let dest_name = format!("{}_{}", sound_id, path.file_name().unwrap_or_default().to_string_lossy());
            let dest = self.sounds_dir.join(dest_name);
            let _ = fs::copy(path, &dest);
            dest.to_string_lossy().to_string()
        } else {
            file_path.to_string()
        };

        let track = self.player.load_sound(
            &sound_id,
            &target_path,
            Some(&final_name),
            volume,
            loop_playback,
        )?;

        let item = SoundItem {
            id: sound_id.clone(),
            name: final_name,
            file_path: target_path,
            hotkey,
            volume,
            loop_playback,
            category: "General".to_string(),
            duration_sec: track.duration_sec,
        };

        self.sounds.write().insert(sound_id, item.clone());
        self.save_to_config();
        Some(item)
    }

    pub fn update_sound(
        &self,
        id: &str,
        volume: Option<f32>,
        loop_playback: Option<bool>,
        hotkey: Option<String>,
    ) -> bool {
        let mut map = self.sounds.write();
        if let Some(item) = map.get_mut(id) {
            if let Some(v) = volume {
                item.volume = v;
            }
            if let Some(lp) = loop_playback {
                item.loop_playback = lp;
            }
            if let Some(hk) = hotkey {
                item.hotkey = if hk.trim().is_empty() { None } else { Some(hk) };
            }
            self.player.update_track(id, volume, loop_playback);
            drop(map);
            self.save_to_config();
            true
        } else {
            false
        }
    }

    pub fn remove_sound(&self, id: &str) -> bool {
        let mut map = self.sounds.write();
        if let Some(item) = map.remove(id) {
            self.player.remove_track(id);
            // If in sounds_dir, delete file
            let p = Path::new(&item.file_path);
            if p.starts_with(&self.sounds_dir) && p.exists() {
                let _ = fs::remove_file(p);
            }
            drop(map);
            self.save_to_config();
            true
        } else {
            false
        }
    }

    pub fn get_all_sounds(&self) -> Vec<SoundItem> {
        self.sounds.read().values().cloned().collect()
    }

    pub fn play_by_hotkey(&self, key: &str) -> bool {
        let normalized = key.trim().to_uppercase();
        let map = self.sounds.read();
        for sound in map.values() {
            if let Some(hk) = &sound.hotkey {
                if hk.trim().to_uppercase() == normalized {
                    let id = sound.id.clone();
                    drop(map);
                    if self.player.is_playing(&id) {
                        self.player.stop(&id);
                    } else {
                        self.player.play(&id);
                    }
                    return true;
                }
            }
        }
        false
    }
}
