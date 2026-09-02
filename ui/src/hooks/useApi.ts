/**
 * pywebview JS → Python köprüsü.
 * window.pywebview hazır olmadan önce çağrı yapılırsa
 * kuyruk mekanizması bekleme sağlar.
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

declare global {
  interface Window {
    pywebview?: {
      api: Record<string, (...args: any[]) => Promise<any>>;
    };
  }
}

function getApi() {
  return window.pywebview?.api ?? null;
}

/**
 * pywebview 5.x'te API metotları `pywebviewready` event'ından sonra kullanılabilir.
 * Event zaten geçmişse doğrudan resolve eder; geçmemişse bekler.
 * Güvenli fallback: 5 saniye sonra polling'e geçer.
 */
let _apiReady: Promise<NonNullable<ReturnType<typeof getApi>>> | null = null;

function waitForApi(): Promise<NonNullable<ReturnType<typeof getApi>>> {
  if (_apiReady) return _apiReady;

  _apiReady = new Promise((resolve) => {
    // Eğer API zaten hazırsa hemen resolve et
    const api = getApi();
    if (api && Object.keys(api).length > 0) {
      resolve(api);
      return;
    }

    // pywebviewready event'ını dinle
    const onReady = () => {
      const readyApi = getApi();
      if (readyApi) resolve(readyApi);
      else poll(); // event geldi ama api hâlâ null ise polling yap
    };
    window.addEventListener("pywebviewready", onReady, { once: true });

    // Fallback polling (500ms aralıklı, 10 deneme)
    let attempts = 0;
    const poll = () => {
      const a = getApi();
      if (a && Object.keys(a).length > 0) {
        window.removeEventListener("pywebviewready", onReady);
        resolve(a);
        return;
      }
      if (attempts++ < 100) setTimeout(poll, 50);
    };
    // pywebviewready gelmezse 500ms sonra polling başlat
    setTimeout(poll, 500);
  });

  return _apiReady;
}

async function call<T = unknown>(method: string, ...args: any[]): Promise<T> {
  const api = await waitForApi();
  if (typeof api[method] !== "function") {
    throw new Error(`API method '${method}' not found`);
  }
  return api[method](...args) as Promise<T>;
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

export const api = {
  getState: () => call<AppState>("get_state"),
  setEngineActive: (active: boolean) => call<{ ok: boolean; active: boolean }>("set_engine_active", active),
  setMuted: (muted: boolean) => call<void>("set_muted", muted),
  setHearMyself: (enabled: boolean) => call<void>("set_hear_myself", enabled),
  setHearSoundboard: (enabled: boolean) => call<void>("set_hear_soundboard", enabled),

  getMeters: () => call<Meters>("get_meters"),

  getPresets: () => call<{ presets: Record<string, PresetConfig>; active: string }>("get_presets"),
  applyPreset: (name: string) => call<{ ok: boolean; active: string }>("apply_preset", name),
  updateDsp: (opts: Partial<PresetConfig>) => call<void>("update_dsp", opts),
  resetPreset: (name: string) =>
    call<{ ok: boolean; presets?: Record<string, PresetConfig>; config?: PresetConfig; error?: string }>("reset_preset", name),
  createPreset: (name: string, config: PresetConfig) =>
    call<{ ok: boolean; name?: string; presets?: Record<string, PresetConfig>; error?: string }>("create_preset", name, config),
  savePreset: (name: string, config: PresetConfig) =>
    call<{ ok: boolean; presets?: Record<string, PresetConfig>; error?: string }>("save_preset", name, config),
  deletePreset: (name: string) =>
    call<{ ok: boolean; presets?: Record<string, PresetConfig>; active?: string; error?: string }>("delete_preset", name),

  getSounds: () => call<Sound[]>("get_sounds"),
  addSoundFile: () => call<{ ok: boolean; sound?: Sound; cancelled?: boolean; error?: string }>("add_sound_file"),
  addSoundData: (filename: string, base64Data: string) =>
    call<{ ok: boolean; sound?: Sound; error?: string }>("add_sound_data", filename, base64Data),
  playSound: (id: string) => call<void>("play_sound", id),
  pauseSound: (id: string) => call<void>("pause_sound", id),
  stopSound: (id: string) => call<void>("stop_sound", id),
  stopAllSounds: () => call<void>("stop_all_sounds"),
  getAllProgress: () => call<Record<string, { is_playing: boolean; progress: number }>>("get_all_progress"),
  updateSound: (id: string, patch: Partial<Pick<Sound, "volume" | "loop" | "hotkey">>) =>
    call<{ ok: boolean }>("update_sound", id, patch.volume, patch.loop, patch.hotkey),
  removeSound: (id: string) => call<{ ok: boolean }>("remove_sound", id),

  getAudioDevices: () => call<AudioDevicesState>("get_audio_devices"),
  setInputDevice: (index: number) => call<void>("set_input_device", index),
  setMonitorDevice: (index: number | null) => call<void>("set_monitor_device", index),
  setBufferSize: (size: number) => call<void>("set_buffer_size", size),
  setMicGain: (gain: number) => call<void>("set_mic_gain", gain),
  setMonitorGain: (gain: number) => call<void>("set_monitor_gain", gain),

  getHotkeyStatus: () =>
    call<{ has_permission: boolean; hotkeys: Array<{ action: string; key: string }> }>("get_hotkey_status"),
};
