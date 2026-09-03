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
            is_setup: AtomicBool::new(false),
        }
    }

    pub fn is_pipewire_available(&self) -> bool {
        match Command::new("pactl").arg("info").output() {
            Ok(out) => out.status.success(),
            Err(e) => {
                error!("PipeWire check error: {}", e);
                false
            }
        }
    }

    pub fn remove_existing_devices(&self) {
        let output = match Command::new("pactl").args(["list", "short", "modules"]).output() {
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
                    let _ = Command::new("pactl").args(["unload-module", id]).output();
                }
            }
        }
    }

    pub fn setup_virtual_devices(&self) -> bool {
        if !self.is_pipewire_available() {
            warn!("PipeWire / PulseAudio daemon not available via pactl.");
            return false;
        }

        self.remove_existing_devices();
        info!("Creating Audiover virtual audio devices (PipeWire/PulseAudio)...");

        // 1. module-null-sink
        let sink_res = Command::new("pactl")
            .args([
                "load-module",
                "module-null-sink",
                &format!("sink_name={}", self.sink_name),
                &format!("sink_properties=device.description=\"{}\"", self.sink_desc),
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

        // 2. module-remap-source
        let source_res = Command::new("pactl")
            .args([
                "load-module",
                "module-remap-source",
                &format!("source_name={}", self.source_name),
                &format!("master={}.monitor", self.sink_name),
                &format!("source_properties=device.description=\"{}\"", self.source_desc),
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
                let _ = Command::new("pactl").args(["unload-module", &sink_id]).output();
                return false;
            }
            Err(e) => {
                warn!("Could not run pactl for remap-source: {}", e);
                let _ = Command::new("pactl").args(["unload-module", &sink_id]).output();
                return false;
            }
        };

        *self.sink_module_id.lock() = Some(sink_id);
        *self.source_module_id.lock() = Some(source_id);
        self.is_setup.store(true, Ordering::SeqCst);

        true
    }

    pub fn cleanup(&self) {
        if let Some(id) = self.source_module_id.lock().take() {
            let _ = Command::new("pactl").args(["unload-module", &id]).output();
        }
        if let Some(id) = self.sink_module_id.lock().take() {
            let _ = Command::new("pactl").args(["unload-module", &id]).output();
        }
        self.is_setup.store(false, Ordering::SeqCst);
    }
}

impl Drop for AudioRouter {
    fn drop(&mut self) {
        self.cleanup();
    }
}
