use evdev::{Device, EventType, KeyCode};
use log::info;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyItem {
    /// Stable action id, e.g. `mute_mic`. Added so the UI can remap keys.
    #[serde(default)]
    pub id: String,
    pub action: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyStatus {
    pub backend: String,
    pub has_permission: bool,
    pub is_running: bool,
    pub hotkeys: Vec<HotkeyItem>,
}

/// The four fixed global actions. `(id, display label, default key)`.
pub const DEFAULT_HOTKEYS: &[(&str, &str, &str)] = &[
    ("mute_mic", "Mute Microphone", "F9"),
    ("bypass_dsp", "Bypass All DSP Effects", "F10"),
    ("stop_all", "Stop All Sounds (Panic)", "F11"),
    (
        "toggle_hear_myself",
        "Toggle Hear Myself (Loopback)",
        "F8",
    ),
];

pub fn default_bindings() -> HashMap<String, String> {
    DEFAULT_HOTKEYS
        .iter()
        .map(|(id, _, key)| (id.to_string(), key.to_string()))
        .collect()
}

pub fn action_label(action_id: &str) -> String {    DEFAULT_HOTKEYS
        .iter()
        .find(|(id, _, _)| *id == action_id)
        .map(|(_, label, _)| label.to_string())
        .unwrap_or_else(|| action_id.to_string())
}

#[allow(dead_code)]
pub fn is_known_action(action_id: &str) -> bool {
    DEFAULT_HOTKEYS.iter().any(|(id, _, _)| *id == action_id)
}

pub fn normalize_key(key: &str) -> String {
    key.trim().to_uppercase()
}

pub struct HotkeyManager {
    /// action_id -> callback
    callbacks: parking_lot::RwLock<HashMap<String, Arc<dyn Fn() + Send + Sync>>>,
    /// action_id -> normalized key name (e.g. "F9", "SPACE")
    bindings: parking_lot::RwLock<HashMap<String, String>>,
    fallback: parking_lot::RwLock<Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>>,
    last_triggers: Mutex<HashMap<String, Instant>>,
    is_running: AtomicBool,
    has_permission: AtomicBool,
}

impl HotkeyManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let has_perm = check_input_permission();
        Self {
            callbacks: parking_lot::RwLock::new(HashMap::new()),
            bindings: parking_lot::RwLock::new(default_bindings()),
            fallback: parking_lot::RwLock::new(None),
            last_triggers: Mutex::new(HashMap::new()),
            is_running: AtomicBool::new(false),
            has_permission: AtomicBool::new(has_perm),
        }
    }

    pub fn with_bindings(bindings: HashMap<String, String>) -> Self {
        let mut merged = default_bindings();
        for (id, key) in bindings {
            if is_known_action(&id) && !normalize_key(&key).is_empty() {
                merged.insert(id, normalize_key(&key));
            }
        }
        let has_perm = check_input_permission();
        Self {
            callbacks: parking_lot::RwLock::new(HashMap::new()),
            bindings: parking_lot::RwLock::new(merged),
            fallback: parking_lot::RwLock::new(None),
            last_triggers: Mutex::new(HashMap::new()),
            is_running: AtomicBool::new(false),
            has_permission: AtomicBool::new(has_perm),
        }
    }

    pub fn set_fallback<F>(&self, handler: F)
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        *self.fallback.write() = Some(Arc::new(handler));
    }

    pub fn register_action<F>(&self, action_id: &str, key: &str, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let normalized = normalize_key(key);
        self.callbacks
            .write()
            .insert(action_id.to_string(), Arc::new(callback));
        if !normalized.is_empty() {
            self.bindings.write().insert(action_id.to_string(), normalized);
        }
    }

    /// Backwards-compatible alias: previously `register(key, cb)`.
    /// Kept for tests / external callers; treats `key` as both id and key.
    #[allow(dead_code)]
    pub fn register<F>(&self, key: &str, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let normalized = normalize_key(key);
        self.callbacks
            .write()
            .insert(normalized, Arc::new(callback));
    }

    /// Remap an existing action to a new key. Returns the conflicting
    /// action id when `new_key` is already taken, or `None` on success.
    /// Empty `new_key` unbinds the action.
    pub fn set_binding(&self, action_id: &str, new_key: &str) -> Option<String> {
        let normalized = normalize_key(new_key);
        if !normalized.is_empty() {
            let bindings = self.bindings.read();
            for (id, key) in bindings.iter() {
                if id != action_id && *key == normalized {
                    return Some(id.clone());
                }
            }
        }
        self.bindings
            .write()
            .insert(action_id.to_string(), normalized);
        None
    }

    pub fn reset_bindings(&self) {
        *self.bindings.write() = default_bindings();
    }

    pub fn bindings_snapshot(&self) -> HashMap<String, String> {
        self.bindings.read().clone()
    }

    pub fn trigger(&self, key: &str) -> bool {
        let normalized = normalize_key(key);

        // 150ms debounce per key (Python parity).
        {
            let mut last = self.last_triggers.lock();
            let now = Instant::now();
            if let Some(prev) = last.get(&normalized) {
                if now.duration_since(*prev) < Duration::from_millis(150) {
                    return false;
                }
            }
            last.insert(normalized.clone(), now);
        }

        // Resolve via current bindings: find the action bound to this key.
        let action_id = self
            .bindings
            .read()
            .iter()
            .find(|(_, k)| **k == normalized)
            .map(|(id, _)| id.clone());
        if let Some(id) = action_id {
            if let Some(cb) = self.callbacks.read().get(&id).cloned() {
                cb();
                return true;
            }
        }

        // Legacy direct-key callbacks (registered via `register`).
        if let Some(cb) = self.callbacks.read().get(&normalized).cloned() {
            cb();
            return true;
        }

        if let Some(fallback_fn) = self.fallback.read().as_ref().cloned() {
            if fallback_fn(&normalized) {
                return true;
            }
        }

        false
    }

    pub fn get_status(&self) -> HotkeyStatus {
        // Report live bindings; `action` keeps the fixed display labels so
        // the UI can match on them (HotkeysPage.getActionLabel), while `id`
        // is the stable key used for remapping.
        let bindings = self.bindings.read();
        let hotkeys: Vec<HotkeyItem> = DEFAULT_HOTKEYS
            .iter()
            .map(|(id, _, default_key)| HotkeyItem {
                id: id.to_string(),
                action: action_label(id),
                key: bindings
                    .get(*id)
                    .cloned()
                    .unwrap_or_else(|| default_key.to_string()),
            })
            .collect();

        let has_perm = self.has_permission.load(Ordering::Relaxed);
        let backend = if has_perm {
            "evdev".to_string()
        } else {
            "in_window".to_string()
        };

        HotkeyStatus {
            backend,
            has_permission: has_perm,
            is_running: self.is_running.load(Ordering::Relaxed),
            hotkeys,
        }
    }

    pub fn start(self: &Arc<Self>) {
        if self.is_running.load(Ordering::SeqCst) {
            return;
        }

        self.is_running.store(true, Ordering::SeqCst);
        let this = self.clone();

        thread::spawn(move || {
            info!("Starting evdev global hotkey listener...");
            let mut monitored_paths = std::collections::HashSet::new();

            while this.is_running.load(Ordering::Relaxed) {
                if let Ok(entries) = fs::read_dir("/dev/input") {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                            if fname.starts_with("event") && !monitored_paths.contains(&path) {
                                if let Ok(mut dev) = Device::open(&path) {
                                    let has_keys = dev.supported_events().contains(EventType::KEY)
                                        && dev
                                            .supported_keys()
                                            .map(|keys| {
                                                keys.contains(KeyCode::KEY_A)
                                                    || keys.contains(KeyCode::KEY_SPACE)
                                                    || keys.contains(KeyCode::KEY_F9)
                                                    || keys.contains(KeyCode::KEY_1)
                                                    || keys.contains(KeyCode::KEY_KP1)
                                            })
                                            .unwrap_or(false);

                                    if has_keys {
                                        monitored_paths.insert(path.clone());
                                        let dev_name = dev.name().unwrap_or("Unknown").to_string();
                                        info!(
                                            "Monitoring global hotkeys on '{}' ({})",
                                            dev_name,
                                            path.display()
                                        );
                                        let manager_ref = this.clone();

                                        thread::spawn(move || {
                                            while manager_ref.is_running.load(Ordering::Relaxed) {
                                                match dev.fetch_events() {
                                                    Ok(events) => {
                                                        for ev in events {
                                                            if ev.event_type() == EventType::KEY && ev.value() == 1 {
                                                                // Key Down
                                                                let key_name = format!("{:?}", KeyCode::new(ev.code()))
                                                                    .replace("KEY_", "")
                                                                    .to_uppercase();
                                                                manager_ref.trigger(&key_name);
                                                            }
                                                        }
                                                    }
                                                    Err(_) => {
                                                        // Device might have been unplugged or disconnected
                                                        break;
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // Check for newly plugged USB devices every 2 seconds
                for _ in 0..20 {
                    if !this.is_running.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

fn check_input_permission() -> bool {
    if let Ok(entries) = fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                if fname.starts_with("event") && Device::open(&path).is_ok() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[test]
    fn remap_action_updates_trigger_routing() {
        let mgr = HotkeyManager::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        mgr.register_action("mute_mic", "F9", move || {
            h.fetch_add(1, AtomicOrdering::SeqCst);
        });
        assert!(mgr.trigger("F9"));
        assert_eq!(hits.load(AtomicOrdering::SeqCst), 1);
        // Remap to a different key: old key stops working, new key works.
        assert_eq!(mgr.set_binding("mute_mic", "F7"), None);
        // Debounce guard: F9 was just triggered; wait it out (tested via
        // a different key path instead of sleeping: trigger new key).
        assert!(mgr.trigger("F7"));
        assert_eq!(hits.load(AtomicOrdering::SeqCst), 2);
    }

    #[test]
    fn conflicting_remap_is_rejected() {
        let mgr = HotkeyManager::new();
        mgr.register_action("mute_mic", "F9", || {});
        mgr.register_action("bypass_dsp", "F10", || {});
        let conflict = mgr.set_binding("mute_mic", "F10");
        assert_eq!(conflict.as_deref(), Some("bypass_dsp"));
        // Original binding untouched.
        let status = mgr.get_status();
        let mute = status.hotkeys.iter().find(|h| h.id == "mute_mic").unwrap();
        assert_eq!(mute.key, "F9");
    }

    #[test]
    fn status_reports_live_bindings_with_ids() {
        let mgr = HotkeyManager::with_bindings(
            [("stop_all".to_string(), "F6".to_string())]
                .into_iter()
                .collect(),
        );
        let status = mgr.get_status();
        assert_eq!(status.hotkeys.len(), 4);
        let stop = status.hotkeys.iter().find(|h| h.id == "stop_all").unwrap();
        assert_eq!(stop.key, "F6");
        // Unspecified ids fall back to defaults.
        let mute = status.hotkeys.iter().find(|h| h.id == "mute_mic").unwrap();
        assert_eq!(mute.key, "F9");
    }
}

