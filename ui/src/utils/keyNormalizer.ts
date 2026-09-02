/**
 * Normalizes a browser KeyboardEvent into an Audiover-standard key name
 * compatible with Linux key names (e.g. F9, SPACE, KPENTER, 1, Q, etc.).
 */
export function normalizeKey(e: KeyboardEvent): string | null {
  // Ignore lone modifier keys
  if (["Control", "Shift", "Alt", "Meta", "AltGraph", "OS"].includes(e.key)) {
    return null;
  }

  const code = e.code;

  // Function keys F1 - F12
  if (/^F([1-9]|1[0-2])$/.test(code)) {
    return code;
  }

  // Digits 0 - 9
  if (/^Digit([0-9])$/.test(code)) {
    return code.replace("Digit", "");
  }

  // Letters A - Z
  if (/^Key([A-Z])$/i.test(code)) {
    return code.replace("Key", "").toUpperCase();
  }

  // Numpad
  if (/^Numpad([0-9])$/.test(code)) {
    return "KP" + code.replace("Numpad", "");
  }
  if (code === "NumpadAdd") return "KPPLUS";
  if (code === "NumpadSubtract") return "KPMINUS";
  if (code === "NumpadMultiply") return "KPASTERISK";
  if (code === "NumpadDivide") return "KPSLASH";
  if (code === "NumpadEnter") return "KPENTER";
  if (code === "NumpadDecimal") return "KPDOT";

  // Common keys
  if (code === "Space" || e.key === " ") return "SPACE";
  if (code === "Tab" || e.key === "Tab") return "TAB";
  if (code === "Enter" || e.key === "Enter") return "ENTER";
  if (code === "Backspace" || e.key === "Backspace") return "BACKSPACE";
  if (code === "Delete" || e.key === "Delete") return "DELETE";
  if (code === "Insert" || e.key === "Insert") return "INSERT";
  if (code === "Home" || e.key === "Home") return "HOME";
  if (code === "End" || e.key === "End") return "END";
  if (code === "PageUp" || e.key === "PageUp") return "PAGEUP";
  if (code === "PageDown" || e.key === "PageDown") return "PAGEDOWN";
  if (code === "ArrowUp" || e.key === "ArrowUp") return "UP";
  if (code === "ArrowDown" || e.key === "ArrowDown") return "DOWN";
  if (code === "ArrowLeft" || e.key === "ArrowLeft") return "LEFT";
  if (code === "ArrowRight" || e.key === "ArrowRight") return "RIGHT";
  if (code === "Minus" || e.key === "-") return "MINUS";
  if (code === "Equal" || e.key === "=") return "EQUAL";
  if (code === "CapsLock" || e.key === "CapsLock") return "CAPSLOCK";

  // Fallback for single characters (e.g. punctuation, international keys)
  if (e.key && e.key.length === 1) {
    return e.key.toUpperCase();
  }

  return null;
}
