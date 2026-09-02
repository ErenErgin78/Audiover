import { create } from "zustand";
import type { AppState, Meters, PresetConfig, Sound } from "../hooks/useApi";

export type PageId = "voice" | "soundboard" | "audio" | "hotkeys" | "settings";

interface AudioStore {
  // ── App State ──
  language: "tr" | "en";
  engineActive: boolean;
  isMuted: boolean;
  hearMyself: boolean;
  hearSoundboard: boolean;
  activePreset: string;
  presets: Record<string, PresetConfig>;

  // ── VU Meters ──
  meters: Meters;

  // ── Soundboard ──
  sounds: Sound[];

  // ── Navigation ──
  activePage: PageId;

  // ── Loaded ──
  initialized: boolean;

  // ── Actions ──
  initFromState: (state: AppState) => void;
  setLanguage: (lang: "tr" | "en") => void;
  setMeters: (m: Meters) => void;
  setEngineActive: (v: boolean) => void;
  setMuted: (v: boolean) => void;
  setHearMyself: (v: boolean) => void;
  setHearSoundboard: (v: boolean) => void;
  setActivePreset: (name: string) => void;
  setPresets: (p: Record<string, PresetConfig>) => void;
  updatePreset: (name: string, config: PresetConfig) => void;
  setSounds: (s: Sound[]) => void;
  upsertSound: (s: Sound) => void;
  removeSound: (id: string) => void;
  setActivePage: (p: PageId) => void;
}

export const useAudioStore = create<AudioStore>((set) => ({
  language: "tr",
  engineActive: false,
  isMuted: false,
  hearMyself: false,
  hearSoundboard: true,
  activePreset: "Clean",
  presets: {},
  meters: { in_peak: 0, in_rms: 0, out_peak: 0, out_rms: 0 },
  sounds: [],
  activePage: "voice",
  initialized: false,

  initFromState: (state) =>
    set({
      language: (state.language as "tr" | "en") || "tr",
      engineActive: state.engine_active,
      isMuted: state.is_muted,
      hearMyself: state.hear_myself,
      hearSoundboard: state.hear_soundboard,
      activePreset: state.active_preset,
      presets: state.presets,
      initialized: true,
    }),

  setLanguage: (lang) => set({ language: lang }),
  setMeters: (m) => set({ meters: m }),
  setEngineActive: (v) => set({ engineActive: v }),
  setMuted: (v) => set({ isMuted: v }),
  setHearMyself: (v) => set({ hearMyself: v }),
  setHearSoundboard: (v) => set({ hearSoundboard: v }),
  setActivePreset: (name) => set({ activePreset: name }),
  setPresets: (p) => set({ presets: p }),
  updatePreset: (name, config) =>
    set((state) => ({
      presets: {
        ...state.presets,
        [name]: config,
      },
    })),
  setSounds: (s) => set({ sounds: s }),
  upsertSound: (s) =>
    set((state) => {
      const idx = state.sounds.findIndex((x) => x.id === s.id);
      if (idx >= 0) {
        const updated = [...state.sounds];
        updated[idx] = s;
        return { sounds: updated };
      }
      return { sounds: [...state.sounds, s] };
    }),
  removeSound: (id) =>
    set((state) => ({ sounds: state.sounds.filter((s) => s.id !== id) })),
  setActivePage: (p) => set({ activePage: p }),
}));

