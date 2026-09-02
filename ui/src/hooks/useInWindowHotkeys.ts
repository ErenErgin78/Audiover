import { useEffect } from "react";
import { api } from "./useApi";
import { normalizeKey } from "../utils/keyNormalizer";

/**
 * In-Window Hotkey Listener.
 * Captures key presses when the Audiover window is focused.
 * The Rust backend safely debounces events to avoid duplicates.
 */
export function useInWindowHotkeys() {
  useEffect(() => {
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
  }, []);
}

