/**
 * Audiover IPC Bridge
 * Supports Tauri v2 (native Rust backend) & PyWebView fallback
 */

/* eslint-disable @typescript-eslint/no-explicit-any */
import { invoke, isTauri } from "@tauri-apps/api/core";

declare global {
  interface Window {
    pywebview?: {
      api: Record<string, (...args: any[]) => Promise<any>>;
    };
    __TAURI_INTERNALS__?: unknown;
  }
}

function hasTauri(): boolean {
  return typeof window !== "undefined" && (isTauri() || Boolean(window.__TAURI_INTERNALS__));
}

export type BackendKind = "tauri" | "pywebview" | "none";

export function getBackendKind(): BackendKind {
  if (typeof window === "undefined") return "none";
  if (isTauri() || Boolean(window.__TAURI_INTERNALS__)) return "tauri";
  if (window.pywebview?.api && Object.keys(window.pywebview.api).length > 0) return "pywebview";
  return "none";
}

function getPyWebViewApi() {
  return window.pywebview?.api ?? null;
}

let _pywebviewReady: Promise<NonNullable<ReturnType<typeof getPyWebViewApi>>> | null = null;

/**
 * Waits for the PyWebView bridge. REJECTS (instead of hanging forever)
 * when the bridge never appears — e.g. the page was opened in a plain
 * browser with no backend. Callers surface this as a connection error.
 */
function waitForPyWebView(
  timeoutMs = 8000
): Promise<NonNullable<ReturnType<typeof getPyWebViewApi>>> {
  if (_pywebviewReady) return _pywebviewReady;

  _pywebviewReady = new Promise((resolve, reject) => {
    const api = getPyWebViewApi();
    if (api && Object.keys(api).length > 0) {
      resolve(api);
      return;
    }

    let settled = false;
    const onReady = () => {
      const readyApi = getPyWebViewApi();
      if (readyApi && !settled) {
        settled = true;
        window.removeEventListener("pywebviewready", onReady);
        resolve(readyApi);
      }
    };
    window.addEventListener("pywebviewready", onReady);

    // Poll for late-injected bridges (separate from the event path).
    const startedAt = Date.now();
    const poll = () => {
      if (settled) return;
      const a = getPyWebViewApi();
      if (a && Object.keys(a).length > 0) {
        settled = true;
        window.removeEventListener("pywebviewready", onReady);
        resolve(a);
        return;
      }
      if (Date.now() - startedAt > timeoutMs) {
        settled = true;
        window.removeEventListener("pywebviewready", onReady);
        _pywebviewReady = null; // allow a later retry
        reject(
          new Error(
            "Backend bridge not found: no Tauri IPC and no PyWebView API. " +
              "Launch the app via ./run.sh (dev) or the installed Audiover binary — " +
              "opening the UI in a plain browser has no backend to talk to."
          )
        );
        return;
      }
      setTimeout(poll, 100);
    };
    setTimeout(poll, 100);
  });

  return _pywebviewReady;
}

/** Invoke with a timeout so a dead backend can't hang the UI forever. */
async function invokeWithTimeout<T>(fn: () => Promise<T>, timeoutMs: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      fn(),
      new Promise<T>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`Timed out waiting for backend: ${what} (${timeoutMs}ms). Is the backend process still running?`)),
          timeoutMs
        );
      }),
    ]);
  } finally {
    if (timer) clearTimeout(timer);
  }
}

/* ── Typed wrappers ──────────────────────────────────────────── */

export interface AppState {
  engine_active: boolean;
  is_muted: boolean;
  hear_myself: boolean;
  hear_soundboard: boolean;
  mic_gain: number;
  monitor_gain: number;
  active_preset: string;
  presets: Record<string, PresetConfig>;
  hotkey_permission: boolean;
  hotkey_backend?: "portal" | "evdev" | "in_window";
  language?: "tr" | "en" | string;
}

export interface HotkeyStatus {
  backend: "portal" | "evdev" | "in_window";
  has_permission: boolean;
  is_running: boolean;
  hotkeys: Array<{ action: string; key: string }>;
}

export interface PresetConfig {
  pitch: number;
  robot: boolean;
  rfreq: number;
  rmix: number;
  radio: boolean;
  dist: boolean;
  drive: number;
  rev: boolean;
  rsize: number;
  rwet: number;
  chorus: boolean;
  cdepth: number;
  bypass?: boolean;
  gate?: boolean;
  gate_db?: number;
}

export interface Sound {
  id: string;
  name: string;
  file_path: string;
  volume: number;
  loop: boolean;
  hotkey: string;
}

export interface Meters {
  in_peak: number;
  in_rms: number;
  out_peak: number;
  out_rms: number;
  is_muted?: boolean;
  hear_myself?: boolean;
  engine_active?: boolean;
}

export interface AudioDevice {
  index: number;
  name: string;
  is_default: boolean;
}

export interface AudioDevicesState {
  inputs: AudioDevice[];
  outputs: AudioDevice[];
  current_input: number | null;
  current_monitor: number | null;
  block_size: number;
  mic_gain: number;
  monitor_gain: number;
  hear_myself: boolean;
  hear_soundboard: boolean;
}

export interface LogEntry {
  seq: number;
  ts_ms: number;
  level: string;
  target: string;
  message: string;
}

export interface Diagnostics {
  app_version: string;
  engine_active: boolean;
  is_muted: boolean;
  hear_myself: boolean;
  hear_soundboard: boolean;
  mic_gain: number;
  monitor_gain: number;
  block_size: number;
  sample_rate: number;
  selected_input: string | null;
  selected_monitor: string | null;
  input_count: number;
  output_count: number;
  current_input: number | null;
  current_monitor: number | null;
  virtual_sink_found: boolean;
  pactl_available: boolean;
  active_preset: string;
  preset_count: number;
  language: string;
  hotkey_backend: string;
  hotkey_permission: boolean;
  log_entries: number;
  config_path: string;
}

export const api = {
  getState: async (): Promise<AppState> => {
    if (hasTauri()) return invokeWithTimeout(() => invoke<AppState>("get_state"), 8000, "get_state");
    const py = await waitForPyWebView();
    return invokeWithTimeout(() => py.get_state(), 8000, "get_state");
  },

  setLanguage: async (lang: string) => {
    if (hasTauri()) return invoke<{ ok: boolean; language: string }>("set_language", { lang });
    const py = await waitForPyWebView();
    return py.set_language(lang);
  },

  setEngineActive: async (active: boolean) => {
    if (hasTauri()) return invoke<{ ok: boolean; active: boolean }>("set_engine_active", { active });
    const py = await waitForPyWebView();
    return py.set_engine_active(active);
  },

  setMuted: async (muted: boolean): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_muted", { muted });
    const py = await waitForPyWebView();
    return py.set_muted(muted);
  },

  setHearMyself: async (enabled: boolean): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_hear_myself", { enabled });
    const py = await waitForPyWebView();
    return py.set_hear_myself(enabled);
  },

  setHearSoundboard: async (enabled: boolean): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_hear_soundboard", { enabled });
    const py = await waitForPyWebView();
    return py.set_hear_soundboard(enabled);
  },

  getMeters: async (): Promise<Meters> => {
    if (hasTauri()) return invoke<Meters>("get_meters");
    const py = await waitForPyWebView();
    return py.get_meters();
  },

  getPresets: async () => {
    if (hasTauri()) return invoke<{ presets: Record<string, PresetConfig>; active: string }>("get_presets");
    const py = await waitForPyWebView();
    return py.get_presets();
  },

  applyPreset: async (name: string) => {
    if (hasTauri()) return invoke<{ ok: boolean; active: string }>("apply_preset", { name });
    const py = await waitForPyWebView();
    return py.apply_preset(name);
  },

  updateDsp: async (opts: Partial<PresetConfig>): Promise<void> => {
    if (hasTauri()) return invoke<void>("update_dsp", { opts });
    const py = await waitForPyWebView();
    return py.update_dsp(opts);
  },

  resetPreset: async (name: string) => {
    if (hasTauri()) {
      return invoke<{ ok: boolean; presets?: Record<string, PresetConfig>; config?: PresetConfig; error?: string }>(
        "reset_preset",
        { name }
      );
    }
    const py = await waitForPyWebView();
    return py.reset_preset(name);
  },

  createPreset: async (name: string, config: PresetConfig) => {
    if (hasTauri()) {
      return invoke<{ ok: boolean; name?: string; presets?: Record<string, PresetConfig>; error?: string }>(
        "create_preset",
        { name, config }
      );
    }
    const py = await waitForPyWebView();
    return py.create_preset(name, config);
  },

  savePreset: async (name: string, config: PresetConfig) => {
    if (hasTauri()) {
      return invoke<{ ok: boolean; presets?: Record<string, PresetConfig>; error?: string }>(
        "save_preset",
        { name, config }
      );
    }
    const py = await waitForPyWebView();
    return py.save_preset(name, config);
  },

  deletePreset: async (name: string) => {
    if (hasTauri()) {
      return invoke<{ ok: boolean; presets?: Record<string, PresetConfig>; active?: string; error?: string }>(
        "delete_preset",
        { name }
      );
    }
    const py = await waitForPyWebView();
    return py.delete_preset(name);
  },

  getSounds: async (): Promise<Sound[]> => {
    if (hasTauri()) {
      const items = await invoke<any[]>("get_sounds");
      return items.map((s) => ({
        id: s.id,
        name: s.name,
        file_path: s.file_path,
        volume: s.volume ?? 1.0,
        loop: s.loop_playback ?? s.loop ?? false,
        hotkey: s.hotkey ?? "",
      }));
    }
    const py = await waitForPyWebView();
    return py.get_sounds();
  },

  addSoundFile: async () => {
    if (hasTauri()) {
      const res = await invoke<any>("add_sound_file");
      if (res.sound) {
        res.sound.loop = res.sound.loop_playback ?? res.sound.loop ?? false;
        res.sound.hotkey = res.sound.hotkey ?? "";
      }
      return res;
    }
    const py = await waitForPyWebView();
    return py.add_sound_file();
  },

  addSoundData: async (filename: string, base64Data: string) => {
    if (hasTauri()) {
      const res = await invoke<any>("add_sound_data", { filename, base64Data });
      if (res.sound) {
        res.sound.loop = res.sound.loop_playback ?? res.sound.loop ?? false;
        res.sound.hotkey = res.sound.hotkey ?? "";
      }
      return res;
    }
    const py = await waitForPyWebView();
    return py.add_sound_data(filename, base64Data);
  },

  playSound: async (id: string): Promise<void> => {
    if (hasTauri()) return invoke<void>("play_sound", { id });
    const py = await waitForPyWebView();
    return py.play_sound(id);
  },

  pauseSound: async (id: string): Promise<void> => {
    if (hasTauri()) return invoke<void>("pause_sound", { id });
    const py = await waitForPyWebView();
    return py.pause_sound(id);
  },

  stopSound: async (id: string): Promise<void> => {
    if (hasTauri()) return invoke<void>("stop_sound", { id });
    const py = await waitForPyWebView();
    return py.stop_sound(id);
  },

  stopAllSounds: async (): Promise<void> => {
    if (hasTauri()) return invoke<void>("stop_all_sounds");
    const py = await waitForPyWebView();
    return py.stop_all_sounds();
  },

  getAllProgress: async (): Promise<Record<string, { is_playing: boolean; progress: number }>> => {
    if (hasTauri()) return invoke<Record<string, { is_playing: boolean; progress: number }>>("get_all_progress");
    const py = await waitForPyWebView();
    return py.get_all_progress();
  },

  updateSound: async (id: string, patch: Partial<Pick<Sound, "volume" | "loop" | "hotkey">>) => {
    if (hasTauri()) {
      return invoke<{ ok: boolean }>("update_sound", {
        id,
        volume: patch.volume,
        loopVal: patch.loop,
        hotkey: patch.hotkey,
      });
    }
    const py = await waitForPyWebView();
    return py.update_sound(id, patch.volume, patch.loop, patch.hotkey);
  },

  removeSound: async (id: string) => {
    if (hasTauri()) return invoke<{ ok: boolean }>("remove_sound", { id });
    const py = await waitForPyWebView();
    return py.remove_sound(id);
  },

  getAudioDevices: async (): Promise<AudioDevicesState> => {
    if (hasTauri()) return invoke<AudioDevicesState>("get_audio_devices");
    const py = await waitForPyWebView();
    return py.get_audio_devices();
  },

  setInputDevice: async (index: number): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_input_device", { index });
    const py = await waitForPyWebView();
    return py.set_input_device(index);
  },

  setMonitorDevice: async (index: number | null): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_monitor_device", { index });
    const py = await waitForPyWebView();
    return py.set_monitor_device(index);
  },

  setBufferSize: async (size: number): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_buffer_size", { size });
    const py = await waitForPyWebView();
    return py.set_buffer_size(size);
  },

  setMicGain: async (gain: number): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_mic_gain", { gain });
    const py = await waitForPyWebView();
    return py.set_mic_gain(gain);
  },

  setMonitorGain: async (gain: number): Promise<void> => {
    if (hasTauri()) return invoke<void>("set_monitor_gain", { gain });
    const py = await waitForPyWebView();
    return py.set_monitor_gain(gain);
  },

  getHotkeyStatus: async (): Promise<HotkeyStatus> => {
    if (hasTauri()) return invoke<HotkeyStatus>("get_hotkey_status");
    const py = await waitForPyWebView();
    return py.get_hotkey_status();
  },

  triggerHotkey: async (key: string) => {
    if (hasTauri()) return invoke<{ ok: boolean }>("trigger_hotkey", { key });
    const py = await waitForPyWebView();
    return py.trigger_hotkey(key);
  },

  getLogs: async (sinceSeq?: number | null): Promise<LogEntry[]> => {
    if (hasTauri()) {
      return invoke<LogEntry[]>("get_logs", { sinceSeq: sinceSeq ?? null });
    }
    const py = await waitForPyWebView();
    if (typeof py.get_logs !== "function") return [];
    return py.get_logs(sinceSeq ?? null);
  },

  clearLogs: async (): Promise<void> => {
    if (hasTauri()) return invoke<void>("clear_logs");
    const py = await waitForPyWebView();
    if (typeof py.clear_logs === "function") return py.clear_logs();
  },

  getDiagnostics: async (): Promise<Diagnostics | null> => {
    if (hasTauri()) return invoke<Diagnostics>("get_diagnostics");
    const py = await waitForPyWebView();
    if (typeof py.get_diagnostics !== "function") return null;
    return py.get_diagnostics();
  },
};
