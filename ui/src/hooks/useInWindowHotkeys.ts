import { useEffect } from "react";
import { api } from "./useApi";
import { useAudioStore } from "../store/audioStore";
import { normalizeKey } from "../utils/keyNormalizer";

/**
 * Global In-Window Hotkey Listener (Tier 3 Fallback).
 * Captures key presses when the Audiover window is active/focused ONLY if
 * global listeners (Portal or evdev) are not active, preventing double-invocation.
 */
export function useInWindowHotkeys() {
  const hotkeyBackend = useAudioStore((s) => s.hotkeyBackend);

  useEffect(() => {
    // If a global listener is already running (portal or evdev), let it handle the shortcut
    if (hotkeyBackend && hotkeyBackend !== "in_window") {
      return;
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      // Do not trigger hotkeys if user is actively typing in a form input or modal
      const target = e.target as HTMLElement | null;
      if (
        target &&
        (target.isContentEditable ||
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT")
      ) {
        return;
      }

      const key = normalizeKey(e);
      if (key) {
        api.triggerHotkey(key).catch((err) => {
          console.debug("In-window hotkey trigger error:", err);
        });
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [hotkeyBackend]);
}
