use crate::audio::stream::is_virtual_device_name;
use log::{error, info, warn};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct AudioRouter {
    sink_name: String,
    sink_desc: String,
    source_name: String,
    source_desc: String,
    sink_module_id: parking_lot::Mutex<Option<String>>,
    source_module_id: parking_lot::Mutex<Option<String>>,
    cached_physical_sink: parking_lot::Mutex<Option<String>>,
    cached_physical_source: parking_lot::Mutex<Option<String>>,
    is_setup: AtomicBool,
}

impl AudioRouter {
    pub fn new(
        sink_name: &str,
        sink_desc: &str,
        source_name: &str,
        source_desc: &str,
    ) -> Self {
        Self {
            sink_name: sink_name.to_string(),
            sink_desc: sink_desc.to_string(),
            source_name: source_name.to_string(),
            source_desc: source_desc.to_string(),
            sink_module_id: parking_lot::Mutex::new(None),
            source_module_id: parking_lot::Mutex::new(None),
            cached_physical_sink: parking_lot::Mutex::new(None),
            cached_physical_source: parking_lot::Mutex::new(None),
            is_setup: AtomicBool::new(false),
        }
    }

    pub fn is_pipewire_available() -> bool {
        match Command::new("pactl")
            .env("LC_ALL", "C")
            .arg("info")
            .output()
        {
            Ok(out) => out.status.success(),
            Err(e) => {
                error!("PipeWire check error: {}", e);
                false
            }
        }
    }

    /// Queries the current default sink from PipeWire / PulseAudio.
    pub fn get_default_sink() -> Option<String> {
        let out = Command::new("pactl")
            .env("LC_ALL", "C")
            .arg("get-default-sink")
            .output()
            .ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }

    /// Queries the current default source from PipeWire / PulseAudio.
    pub fn get_default_source() -> Option<String> {
        let out = Command::new("pactl")
            .env("LC_ALL", "C")
            .arg("get-default-source")
            .output()
            .ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }

    /// Sets the default output sink via pactl.
    pub fn set_default_sink(sink_name: &str) -> bool {
        match Command::new("pactl")
            .env("LC_ALL", "C")
            .args(["set-default-sink", sink_name])
            .output()
        {
            Ok(out) => {
                if out.status.success() {
                    info!("Restored system default sink to: {}", sink_name);
                    true
                } else {
                    warn!(
                        "Failed to set default sink to '{}': {}",
                        sink_name,
                        String::from_utf8_lossy(&out.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                warn!("Could not run pactl set-default-sink: {}", e);
                false
            }
        }
    }

    /// Sets the default input source via pactl.
    pub fn set_default_source(source_name: &str) -> bool {
        match Command::new("pactl")
            .env("LC_ALL", "C")
            .args(["set-default-source", source_name])
            .output()
        {
            Ok(out) => {
                if out.status.success() {
                    info!("Restored system default source to: {}", source_name);
                    true
                } else {
                    warn!(
                        "Failed to set default source to '{}': {}",
                        source_name,
                        String::from_utf8_lossy(&out.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                warn!("Could not run pactl set-default-source: {}", e);
                false
            }
        }
    }

    /// Resolves the genuine physical default sink, bypassing Audiover virtual sinks
    /// or black-hole pseudo devices even if they were elected by mistake.
    pub fn get_physical_default_sink() -> Option<String> {
        if let Some(def) = Self::get_default_sink() {
            if !is_virtual_device_name(&def) {
                return Some(def);
            }
        }

        // Fallback: iterate available sinks and pick the first physical non-virtual sink
        let output = match Command::new("pactl")
            .env("LC_ALL", "C")
            .args(["list", "short", "sinks"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return None,
        };

        for line in output.lines() {
            let mut parts = line.split_whitespace();
            let _id = parts.next();
            if let Some(name) = parts.next() {
                if !is_virtual_device_name(name) {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// Resolves the genuine physical default source, bypassing Audiover virtual sources
    /// or monitor remaps.
    pub fn get_physical_default_source() -> Option<String> {
        if let Some(def) = Self::get_default_source() {
            if !is_virtual_device_name(&def) && !def.ends_with(".monitor") {
                return Some(def);
            }
        }

        let output = match Command::new("pactl")
            .env("LC_ALL", "C")
            .args(["list", "short", "sources"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return None,
        };

        for line in output.lines() {
            let mut parts = line.split_whitespace();
            let _id = parts.next();
            if let Some(name) = parts.next() {
                if !is_virtual_device_name(name) && !name.ends_with(".monitor") {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// Resolves a PipeWire/PulseAudio sink name from an ALSA or CPAL device name/description.
    /// If no match is found, returns the active physical default sink.
    pub fn resolve_sink_name(name_or_desc: Option<&str>) -> Option<String> {
        if let Some(s) = name_or_desc {
            let query = s.trim().to_lowercase();
            if !query.is_empty()
                && !is_virtual_device_name(&query)
                && query != "none"
                && !query.contains("pipewire")
                && !query.contains("default")
            {
                if let Ok(out) = Command::new("pactl")
                    .env("LC_ALL", "C")
                    .args(["list", "sinks"])
                    .output()
                {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let mut current_sink: Option<String> = None;
                    let mut current_matched = false;

                    for line in text.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("Name: ") {
                            if current_matched {
                                if let Some(sink) = current_sink {
                                    if !is_virtual_device_name(&sink) {
                                        return Some(sink);
                                    }
                                }
                            }
                            let name = trimmed.trim_start_matches("Name: ").trim().to_string();
                            current_matched = name.to_lowercase().contains(&query);
                            current_sink = Some(name);
                        } else if let Some(ref _sink) = current_sink {
                            let lower_line = trimmed.to_lowercase();
                            if lower_line.contains(&query) {
                                current_matched = true;
                            }
                        }
                    }
                    if current_matched {
                        if let Some(sink) = current_sink {
                            if !is_virtual_device_name(&sink) {
                                return Some(sink);
                            }
                        }
                    }
                }
            }
        }
        Self::get_physical_default_sink()
    }

    /// Cleans any stale WirePlumber stream routing rules where Audiover was assigned as target
    pub fn sanitize_wireplumber_state() {
        if let Some(state_dir) = dirs::state_dir().or_else(|| dirs::home_dir().map(|h| h.join(".local/state"))) {
            let wp_dir = state_dir.join("wireplumber");
            if wp_dir.exists() {
                let sp_path = wp_dir.join("stream-properties");
                let dn_path = wp_dir.join("default-nodes");

                let mut needs_clean = false;
                if let Ok(content) = std::fs::read_to_string(&sp_path) {
                    if content.contains("audiover") || content.contains("Audiover") {
                        needs_clean = true;
                    }
                }
                if let Ok(content) = std::fs::read_to_string(&dn_path) {
                    if content.contains("audiover") || content.contains("Audiover") {
                        needs_clean = true;
                    }
                }

                if needs_clean {
                    // WirePlumber flushes in-memory state on exit; stop it first so it won't overwrite our scrub
                    let _ = Command::new("systemctl")
                        .args(["--user", "stop", "wireplumber"])
                        .output();

                    if let Ok(content) = std::fs::read_to_string(&sp_path) {
                        let lines: Vec<&str> = content
                            .lines()
                            .filter(|l| {
                                !(l.contains("audiover") || l.contains("Audiover_Sink") || l.contains("Audiover_Mic"))
                            })
                            .collect();
                        let cleaned = if lines.is_empty() {
                            String::new()
                        } else {
                            lines.join("\n") + "\n"
                        };
                        let _ = std::fs::write(&sp_path, cleaned);
                    }

                    if let Ok(content) = std::fs::read_to_string(&dn_path) {
                        let lines: Vec<&str> = content
                            .lines()
                            .filter(|l| !l.contains("Audiover") && !l.contains("audiover"))
                            .collect();
                        let cleaned = if lines.is_empty() {
                            String::new()
                        } else {
                            lines.join("\n") + "\n"
                        };
                        let _ = std::fs::write(&dn_path, cleaned);
                    }

                    let _ = Command::new("systemctl")
                        .args(["--user", "start", "wireplumber"])
                        .output();
                    info!("Sanitized WirePlumber stream-properties and default-nodes");
                }
            }
        }
    }

    pub fn remove_existing_devices(&self) {
        Self::sanitize_wireplumber_state();
        let output = match Command::new("pactl")
            .env("LC_ALL", "C")
            .args(["list", "short", "modules"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return,
        };

        for line in output.lines() {
            // Match the specific device names plus any generic Audiover
            // leftover (Python parity: crash leftovers are fully cleaned).
            if line.contains(&self.sink_name)
                || line.contains(&self.source_name)
                || line.contains("Audiover")
            {
                if let Some(id) = line.split_whitespace().next() {
                    info!("Unloading leftover virtual audio module ID: {}", id);
                    let _ = Command::new("pactl")
                        .env("LC_ALL", "C")
                        .args(["unload-module", id])
                        .output();
                }
            }
        }
    }

    pub fn setup_virtual_devices(&self) -> bool {
        Self::sanitize_wireplumber_state();
        if !Self::is_pipewire_available() {
            warn!("PipeWire / PulseAudio daemon not available via pactl.");
            return false;
        }

        // Cache genuine physical default endpoints before touching module state
        let phys_sink = Self::get_physical_default_sink();
        let phys_src = Self::get_physical_default_source();
        if let Some(ref s) = phys_sink {
            info!("Cached physical default sink before module load: {}", s);
        }
        if let Some(ref s) = phys_src {
            info!("Cached physical default source before module load: {}", s);
        }
        *self.cached_physical_sink.lock() = phys_sink.clone();
        *self.cached_physical_source.lock() = phys_src.clone();

        self.remove_existing_devices();
        info!("Creating Audiover virtual audio devices (PipeWire/PulseAudio)...");

        // 1. module-null-sink with deprioritization properties:
        // - priority.driver=0 & priority.session=0: WirePlumber/PulseAudio won't elect it as default.
        // - node.passive=true: PipeWire won't drive links unless explicitly routed.
        // - device.class="abstract": Marks it as virtual/abstract device.
        let sink_props = format!(
            "device.description=\"{}\" device.class=\"abstract\" node.passive=true priority.driver=0 priority.session=0",
            self.sink_desc
        );
        let sink_res = Command::new("pactl")
            .env("LC_ALL", "C")
            .args([
                "load-module",
                "module-null-sink",
                &format!("sink_name={}", self.sink_name),
                &format!("sink_properties={}", sink_props),
            ])
            .output();

        let sink_id = match sink_res {
            Ok(out) if out.status.success() => {
                let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
                info!("Loaded Virtual Sink '{}' (Module ID: {})", self.sink_name, id);
                id
            }
            Ok(out) => {
                warn!("pactl sink load failed: {}", String::from_utf8_lossy(&out.stderr));
                return false;
            }
            Err(e) => {
                warn!("Could not run pactl: {}", e);
                return false;
            }
        };

        // 2. module-remap-source with deprioritization properties:
        // - priority.driver=1 & priority.session=1: Deprioritized below physical microphones.
        // - device.class="abstract": Marks it as virtual source.
        let src_props = format!(
            "device.description=\"{}\" device.class=\"abstract\" priority.driver=1 priority.session=1",
            self.source_desc
        );
        let source_res = Command::new("pactl")
            .env("LC_ALL", "C")
            .args([
                "load-module",
                "module-remap-source",
                &format!("source_name={}", self.source_name),
                &format!("master={}.monitor", self.sink_name),
                &format!("source_properties={}", src_props),
            ])
            .output();

        let source_id = match source_res {
            Ok(out) if out.status.success() => {
                let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
                info!("Loaded Virtual Mic '{}' (Module ID: {})", self.source_name, id);
                id
            }
            Ok(out) => {
                warn!("pactl source load failed: {}", String::from_utf8_lossy(&out.stderr));
                let _ = Command::new("pactl").env("LC_ALL", "C").args(["unload-module", &sink_id]).output();
                return false;
            }
            Err(e) => {
                warn!("Could not run pactl for remap-source: {}", e);
                let _ = Command::new("pactl").env("LC_ALL", "C").args(["unload-module", &sink_id]).output();
                return false;
            }
        };

        // 3. Post-load Default Sink / Source Verification & Restoration:
        // Enforce physical endpoints as system defaults so virtual devices never steal output.
        if let Some(ref target_sink) = phys_sink {
            let cur_sink = Self::get_default_sink();
            if cur_sink.as_deref() == Some(&self.sink_name)
                || cur_sink.as_ref().map(|s| s.to_lowercase().contains("audiover")).unwrap_or(false)
            {
                warn!(
                    "Default sink was hijacked by '{:?}'. Restoring physical sink '{}'...",
                    cur_sink, target_sink
                );
            }
            let _ = Self::set_default_sink(target_sink);
        }

        if let Some(ref target_src) = phys_src {
            let cur_src = Self::get_default_source();
            if cur_src.as_deref() == Some(&self.source_name)
                || cur_src.as_ref().map(|s| s.to_lowercase().contains("audiover")).unwrap_or(false)
            {
                warn!(
                    "Default source was hijacked by '{:?}'. Restoring physical source '{}'...",
                    cur_src, target_src
                );
            }
            let _ = Self::set_default_source(target_src);
        }

        *self.sink_module_id.lock() = Some(sink_id);
        *self.source_module_id.lock() = Some(source_id);
        self.is_setup.store(true, Ordering::SeqCst);

        true
    }

    pub fn cleanup(&self) {
        // If the system default sink is still pointing to Audiover_Sink, restore physical sink before unloading
        if let Some(cur_sink) = Self::get_default_sink() {
            if cur_sink == self.sink_name || cur_sink.to_lowercase().contains("audiover") {
                if let Some(ref target_sink) = *self.cached_physical_sink.lock() {
                    let _ = Self::set_default_sink(target_sink);
                }
            }
        }
        if let Some(cur_src) = Self::get_default_source() {
            if cur_src == self.source_name || cur_src.to_lowercase().contains("audiover") {
                if let Some(ref target_src) = *self.cached_physical_source.lock() {
                    let _ = Self::set_default_source(target_src);
                }
            }
        }

        if let Some(id) = self.source_module_id.lock().take() {
            let _ = Command::new("pactl").env("LC_ALL", "C").args(["unload-module", &id]).output();
        }
        if let Some(id) = self.sink_module_id.lock().take() {
            let _ = Command::new("pactl").env("LC_ALL", "C").args(["unload-module", &id]).output();
        }
        self.is_setup.store(false, Ordering::SeqCst);
    }
}

impl Drop for AudioRouter {
    fn drop(&mut self) {
        self.cleanup();
    }
}
