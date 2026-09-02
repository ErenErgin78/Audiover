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

pub struct HotkeyManager {
    callbacks: parking_lot::RwLock<HashMap<String, Arc<dyn Fn() + Send + Sync>>>,
    fallback: parking_lot::RwLock<Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>>,
    last_triggers: Mutex<HashMap<String, Instant>>,
    is_running: AtomicBool,
    has_permission: AtomicBool,
}

impl HotkeyManager {
    pub fn new() -> Self {
        let has_perm = check_input_permission();
        Self {
            callbacks: parking_lot::RwLock::new(HashMap::new()),
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

    pub fn register<F>(&self, key: &str, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let normalized = key.trim().to_uppercase();
        self.callbacks
            .write()
            .insert(normalized, Arc::new(callback));
    }

    pub fn trigger(&self, key: &str) -> bool {
        let normalized = key.trim().to_uppercase();

        // 80ms debounce per key
        {
            let mut last = self.last_triggers.lock();
            let now = Instant::now();
            if let Some(prev) = last.get(&normalized) {
                if now.duration_since(*prev) < Duration::from_millis(80) {
                    return false;
                }
            }
            last.insert(normalized.clone(), now);
        }

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
        let keys = self.callbacks.read();
        let hotkeys = keys
            .keys()
            .map(|k| HotkeyItem {
                action: k.clone(),
                key: k.clone(),
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

