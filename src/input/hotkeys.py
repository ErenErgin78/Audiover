import glob
import logging
import os
import struct
import threading
import time
from typing import Callable, Dict, List, Optional, Set

logger = logging.getLogger("Audiover.HotkeyManager")

# Linux Input Event struct format: struct timeval (16 bytes on 64-bit), uint16 type, uint16 code, int32 value
# Total size: 24 bytes
EVENT_FORMAT = "llHHI"
EVENT_SIZE = struct.calcsize(EVENT_FORMAT)

EV_KEY = 1
KEY_UP = 0
KEY_DOWN = 1
KEY_HOLD = 2

# Standard Linux Keycode Mapping
LINUX_KEY_MAP = {
    1: "ESC",
    2: "1",
    3: "2",
    4: "3",
    5: "4",
    6: "5",
    7: "6",
    8: "7",
    9: "8",
    10: "9",
    11: "0",
    12: "MINUS",
    13: "EQUAL",
    14: "BACKSPACE",
    15: "TAB",
    16: "Q",
    17: "W",
    18: "E",
    19: "R",
    20: "T",
    21: "Y",
    22: "U",
    23: "I",
    24: "O",
    25: "P",
    29: "CTRL",
    30: "A",
    31: "S",
    32: "D",
    33: "F",
    34: "G",
    35: "H",
    36: "J",
    37: "K",
    38: "L",
    42: "SHIFT",
    44: "Z",
    45: "X",
    46: "C",
    47: "V",
    48: "B",
    49: "N",
    50: "M",
    56: "ALT",
    57: "SPACE",
    58: "CAPSLOCK",
    59: "F1",
    60: "F2",
    61: "F3",
    62: "F4",
    63: "F5",
    64: "F6",
    65: "F7",
    66: "F8",
    67: "F9",
    68: "F10",
    69: "NUMLOCK",
    70: "SCROLLLOCK",
    71: "KP7",
    72: "KP8",
    73: "KP9",
    74: "KPMINUS",
    75: "KP4",
    76: "KP5",
    77: "KP6",
    78: "KPPLUS",
    79: "KP1",
    80: "KP2",
    81: "KP3",
    82: "KP0",
    83: "KPDOT",
    87: "F11",
    88: "F12",
    96: "KPENTER",
    97: "RCTRL",
    98: "KPSLASH",
    100: "RALT",
    102: "HOME",
    103: "UP",
    104: "PAGEUP",
    105: "LEFT",
    106: "RIGHT",
    107: "END",
    108: "DOWN",
    109: "PAGEDOWN",
    110: "INSERT",
    111: "DELETE",
    125: "SUPER",
}

REV_KEY_MAP = {v.upper(): k for k, v in LINUX_KEY_MAP.items()}


class HotkeyManager:
    """Unified Global Hotkey Manager supporting Wayland (/dev/input), X11, and Qt fallbacks."""

    def __init__(self):
        self.registered_hotkeys: Dict[str, Callable[[], None]] = {}
        self.is_running = False
        self.has_global_permissions = False
        self._threads: List[threading.Thread] = []
        self._file_descriptors: List[int] = []
        self._pressed_keys: Set[str] = set()
        self._lock = threading.Lock()

        # Check permissions
        self.check_permissions()

    def check_permissions(self) -> bool:
        """Tests if the current process can read from /dev/input devices."""
        device_paths = glob.glob("/dev/input/event*")
        accessible = 0
        for path in device_paths:
            try:
                fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
                os.close(fd)
                accessible += 1
            except (PermissionError, OSError):
                pass

        self.has_global_permissions = accessible > 0
        return self.has_global_permissions

    def register_hotkey(self, hotkey_str: str, callback: Callable[[], None]):
        """Registers a callback for a given hotkey string (e.g. 'F8', 'F9', 'Ctrl+Shift+1')."""
        if not hotkey_str or not hotkey_str.strip():
            return
        clean_key = hotkey_str.strip().upper()
        with self._lock:
            self.registered_hotkeys[clean_key] = callback
            logger.info(f"Registered hotkey: {clean_key}")

    def unregister_hotkey(self, hotkey_str: str):
        if not hotkey_str:
            return
        clean_key = hotkey_str.strip().upper()
        with self._lock:
            if clean_key in self.registered_hotkeys:
                del self.registered_hotkeys[clean_key]

    def clear_hotkeys(self):
        with self._lock:
            self.registered_hotkeys.clear()

    def start(self):
        """Starts background hotkey listeners."""
        if self.is_running:
            return

        self.is_running = True
        self.check_permissions()

        if self.has_global_permissions:
            self._start_evdev_listeners()
        else:
            logger.warning(
                "No direct read access to /dev/input. Global hotkeys outside the window may require adding user to 'input' group (sudo usermod -aG input $USER)."
            )

    def stop(self):
        """Stops all background listeners."""
        self.is_running = False
        for fd in self._file_descriptors:
            try:
                os.close(fd)
            except Exception:
                pass
        self._file_descriptors.clear()
        self._threads.clear()

    def _start_evdev_listeners(self):
        """Opens readable keyboard event devices and reads them via raw input_event structs."""
        device_paths = glob.glob("/dev/input/event*")
        keyboard_fds = []

        for path in device_paths:
            try:
                fd = os.open(path, os.O_RDONLY)
                keyboard_fds.append((path, fd))
            except (PermissionError, OSError):
                pass

        for path, fd in keyboard_fds:
            self._file_descriptors.append(fd)
            t = threading.Thread(
                target=self._device_reader_loop,
                args=(path, fd),
                daemon=True,
                name=f"HotkeyListener-{os.path.basename(path)}",
            )
            t.start()
            self._threads.append(t)

        logger.info(
            f"Started {len(keyboard_fds)} /dev/input event reader threads."
        )

    def _device_reader_loop(self, path: str, fd: int):
        """Reads raw Linux input_event structs continuously."""
        while self.is_running:
            try:
                data = os.read(fd, EVENT_SIZE * 16)
                if not data:
                    break

                for i in range(0, len(data), EVENT_SIZE):
                    chunk = data[i : i + EVENT_SIZE]
                    if len(chunk) < EVENT_SIZE:
                        continue

                    # Unpack struct input_event
                    _, _, ev_type, code, value = struct.unpack(
                        EVENT_FORMAT, chunk
                    )

                    if ev_type == EV_KEY:
                        key_name = LINUX_KEY_MAP.get(code, f"KEY_{code}")
                        if value == KEY_DOWN:
                            self._handle_key_down(key_name)
                        elif value == KEY_UP:
                            self._handle_key_up(key_name)
            except OSError:
                break
            except Exception as e:
                logger.debug(f"Error reading event from {path}: {e}")
                time.sleep(0.01)

    def _handle_key_down(self, key_name: str):
        with self._lock:
            self._pressed_keys.add(key_name)
            active_combo = "+".join(sorted(list(self._pressed_keys)))

            # Check single key match (e.g. "F8", "F9")
            callback = self.registered_hotkeys.get(
                key_name
            ) or self.registered_hotkeys.get(active_combo)

        if callback:
            try:
                logger.info(f"Triggering hotkey action for: {key_name}")
                callback()
            except Exception as e:
                logger.error(f"Error in hotkey callback: {e}")

    def _handle_key_up(self, key_name: str):
        with self._lock:
            self._pressed_keys.discard(key_name)

    def trigger_hotkey_manually(self, key_str: str):
        """Allows manual invocation from UI or Qt key events."""
        clean = key_str.strip().upper()
        with self._lock:
            callback = self.registered_hotkeys.get(clean)
        if callback:
            callback()
