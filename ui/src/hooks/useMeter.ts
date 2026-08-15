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
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    let active = true;

    const poll = async () => {
      if (!active) return;
      try {
        const meters: Meters = await api.getMeters();
        if (active) setMeters(meters);
      } catch {
        // pywebview hazır değilse sessizce geç
      }
    };

    intervalRef.current = setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      active = false;
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [setMeters]);
}
