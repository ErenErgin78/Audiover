import { useEffect, useRef } from "react";
import { api, type Meters } from "./useApi";
import { useAudioStore } from "../store/audioStore";

const POLL_INTERVAL_MS = 40; // ~25 FPS

/**
 * VU meter verilerini 40ms aralıkla pywebview'dan çekerek
 * Zustand store'una yazar.
 */
export function useMeter() {
  const setMeters = useAudioStore((s) => s.setMeters);
  const setMuted = useAudioStore((s) => s.setMuted);
  const setHearMyself = useAudioStore((s) => s.setHearMyself);
  const setEngineActive = useAudioStore((s) => s.setEngineActive);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    let active = true;

    const poll = async () => {
      if (!active) return;
      try {
        const meters: Meters = await api.getMeters();
        if (active) {
          setMeters(meters);

          // Synchronize state toggled by hotkeys (F9 Mute, F8 Hear Myself, etc.)
          if (typeof meters.is_muted === "boolean") {
            const currentMuted = useAudioStore.getState().isMuted;
            if (currentMuted !== meters.is_muted) {
              setMuted(meters.is_muted);
            }
          }
          if (typeof meters.hear_myself === "boolean") {
            const currentHear = useAudioStore.getState().hearMyself;
            if (currentHear !== meters.hear_myself) {
              setHearMyself(meters.hear_myself);
            }
          }
          if (typeof meters.engine_active === "boolean") {
            const currentActive = useAudioStore.getState().engineActive;
            if (currentActive !== meters.engine_active) {
              setEngineActive(meters.engine_active);
            }
          }
        }
      } catch {
        // pywebview hazır değilse sessizce geç
      }
    };

    intervalRef.current = setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      active = false;
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [setMeters, setMuted, setHearMyself, setEngineActive]);
}
