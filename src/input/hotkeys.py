import enum
import glob
import logging
import os
import platform
import struct
import threading
import time
from typing import Callable, Dict, List, Optional, Set

logger = logging.getLogger("Audiover.HotkeyManager")

# Architecture-independent Linux Input Event struct format
# struct timeval (tv_sec, tv_usec), uint16 type, uint16 code, int32 value
IS_64BIT = struct.calcsize("P") == 8
EVENT_FORMAT = "qqHHi" if IS_64BIT else "iiHHi"
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


class HotkeyBackend(str, enum.Enum):
    PORTAL = "portal"
    EVDEV = "evdev"
    IN_WINDOW = "in_window"


def _ensure_desktop_file_registered():
    """Ensures audiover.desktop exists in user applications directory for XDG portal app_id resolution."""
    dest_dir = os.path.expanduser("~/.local/share/applications")
    dest_file = os.path.join(dest_dir, "audiover.desktop")
    if os.path.exists(dest_file):
        return

    # Look for bundled audiover.desktop
    possible_paths = [
        os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "packaging", "desktop", "audiover.desktop"),
        os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "packaging", "desktop", "audiover.desktop"),
        "/usr/share/applications/audiover.desktop",
    ]
    for src in possible_paths:
        if os.path.isfile(src):
            try:
                os.makedirs(dest_dir, exist_ok=True)
                import shutil
                shutil.copy2(src, dest_file)
                logger.debug(f"Copied desktop file to {dest_file}")
                return
            except Exception as e:
                logger.debug(f"Could not copy desktop file: {e}")

    # If no source file found, create a minimal desktop file
    try:
        os.makedirs(dest_dir, exist_ok=True)
        with open(dest_file, "w", encoding="utf-8") as f:
            f.write(
                "[Desktop Entry]\n"
                "Name=Audiover\n"
                "Exec=audiover %U\n"
                "Icon=audiover\n"
                "Type=Application\n"
                "Categories=AudioVideo;Audio;\n"
            )
    except Exception as e:
        logger.debug(f"Could not write default desktop file: {e}")


class XDGPortalHotkeyListener:
    """
    Tier 1 Hotkey Listener using XDG Desktop Portal GlobalShortcuts D-Bus interface.
    Works on modern Wayland (GNOME, KDE) sessions without root or input group privileges.
    """

    def __init__(self, on_trigger: Callable[[str], None]):
        self.on_trigger = on_trigger
        self.is_active = False
        self._session_handle: Optional[str] = None
        self._bus = None
        self._loop = None
        self._thread: Optional[threading.Thread] = None
        self._registered_keys: Set[str] = set()
        self._lock = threading.Lock()

    def start(self) -> bool:
        try:
            import dbus
            from dbus.mainloop.glib import DBusGMainLoop
            from gi.repository import GLib

            DBusGMainLoop(set_as_default=True)
            self._bus = dbus.SessionBus()

            # 1. Check if portal service is accessible
            portal_obj = self._bus.get_object("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop")
            introspect_iface = dbus.Interface(portal_obj, "org.freedesktop.DBus.Introspectable")
            xml_data = introspect_iface.Introspect()

            if "org.freedesktop.portal.GlobalShortcuts" not in xml_data:
                logger.debug("XDG GlobalShortcuts interface not available in portal.")
                return False

            # 2. Ensure application is registered with Registry interface if required
            _ensure_desktop_file_registered()
            if "org.freedesktop.host.portal.Registry" in xml_data:
                try:
                    reg_iface = dbus.Interface(portal_obj, "org.freedesktop.host.portal.Registry")
                    reg_iface.Register("audiover", {})
                except Exception as reg_err:
                    logger.debug(f"Portal Registry.Register notice: {reg_err}")

            self._shortcuts_iface = dbus.Interface(portal_obj, "org.freedesktop.portal.GlobalShortcuts")

            # 3. Create portal session
            pid = os.getpid()
            session_token = f"audiover_s_{pid}_{int(time.time())}"
            handle_token = f"audiover_r_{pid}_{int(time.time())}"

            bus_name = self._bus.get_unique_name().lstrip(":").replace(".", "_")
            req_path = f"/org/freedesktop/portal/desktop/request/{bus_name}/{handle_token}"

            session_ready_event = threading.Event()
            session_success = False

            def on_session_response(response, results):
                nonlocal session_success
                if response == 0:
                    self._session_handle = str(results.get("session_handle", ""))
                    session_success = bool(self._session_handle)
                    logger.info(f"XDG Portal GlobalShortcuts session created: {self._session_handle}")
                else:
                    logger.debug(f"XDG Portal CreateSession response error code: {response}")
                session_ready_event.set()

            self._bus.add_signal_receiver(
                on_session_response,
                signal_name="Response",
                dbus_interface="org.freedesktop.portal.Request",
                path=req_path,
            )

            # Listen for Activated signals
            def on_shortcut_activated(session_handle, shortcut_id, timestamp, options):
                if self._session_handle and str(session_handle) == self._session_handle:
                    logger.info(f"XDG Portal shortcut activated: {shortcut_id}")
                    self.on_trigger(str(shortcut_id))

            self._bus.add_signal_receiver(
                on_shortcut_activated,
                signal_name="Activated",
                dbus_interface="org.freedesktop.portal.GlobalShortcuts",
                path="/org/freedesktop/portal/desktop",
            )

            # Call CreateSession
            self._shortcuts_iface.CreateSession({
                "session_handle_token": session_token,
                "handle_token": handle_token,
            })

            # Start GLib main loop in background thread
            self._loop = GLib.MainLoop()
            self._thread = threading.Thread(
                target=self._loop.run,
                daemon=True,
                name="XDGPortalMainLoop",
            )
            self._thread.start()

            # Wait briefly for session creation response
            session_ready_event.wait(timeout=1.5)

            if session_success and self._session_handle:
                self.is_active = True
                return True
            else:
                logger.debug("XDG Portal session response timed out or was rejected.")
                self.stop()
                return False

        except Exception as e:
            logger.debug(f"Could not initialize XDG Portal GlobalShortcuts: {e}")
            self.stop()
            return False

    def sync_shortcuts(self, keys: Set[str]):
        """Binds registered keys with the active portal session."""
        if not self.is_active or not self._session_handle or not self._shortcuts_iface:
            return

        with self._lock:
            if keys == self._registered_keys:
                return
            self._registered_keys = set(keys)

        try:
            pid = os.getpid()
            bind_token = f"audiover_b_{pid}_{int(time.time() * 1000) % 1000000}"
            shortcuts_list = []
            for key in keys:
                shortcuts_list.append((
                    key,
                    {
                        "description": f"Audiover shortcut: {key}",
                        "preferred_trigger": key,
                    },
                ))

            self._shortcuts_iface.BindShortcuts(
                self._session_handle,
                shortcuts_list,
                "",  # parent window
                {"handle_token": bind_token},
            )
            logger.debug(f"Requested XDG Portal BindShortcuts for: {list(keys)}")
        except Exception as e:
            logger.debug(f"Failed to sync shortcuts with XDG Portal: {e}")

    def stop(self):
        self.is_active = False
        if self._session_handle and self._bus:
            try:
                session_obj = self._bus.get_object("org.freedesktop.portal.Desktop", self._session_handle)
                session_iface = dbus.Interface(session_obj, "org.freedesktop.portal.Session")
                session_iface.Close()
            except Exception:
                pass
            self._session_handle = None

        if self._loop and self._loop.is_running():
            try:
                self._loop.quit()
            except Exception:
                pass


class EvdevHotkeyListener:
    """
    Tier 2 Hotkey Listener using /dev/input Linux evdev structures.
    Works when user has read permissions on input event character devices.
    """

    def __init__(self, on_key_down: Callable[[str], None], on_key_up: Callable[[str], None]):
        self.on_key_down = on_key_down
        self.on_key_up = on_key_up
        self.is_running = False
        self._threads: List[threading.Thread] = []
        self._file_descriptors: List[int] = []

    def check_permissions(self) -> bool:
        device_paths = glob.glob("/dev/input/event*")
        accessible = 0
        for path in device_paths:
            try:
                fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
                os.close(fd)
                accessible += 1
            except (PermissionError, OSError):
                pass
        return accessible > 0

    def start(self) -> bool:
        if not self.check_permissions():
            return False

        device_paths = glob.glob("/dev/input/event*")
        keyboard_fds = []
        for path in device_paths:
            try:
                fd = os.open(path, os.O_RDONLY)
                keyboard_fds.append((path, fd))
            except (PermissionError, OSError):
                pass

        if not keyboard_fds:
            return False

        self.is_running = True
        for path, fd in keyboard_fds:
            self._file_descriptors.append(fd)
            t = threading.Thread(
                target=self._device_reader_loop,
                args=(path, fd),
                daemon=True,
                name=f"EvdevHotkeyListener-{os.path.basename(path)}",
            )
            t.start()
            self._threads.append(t)

        logger.info(f"Started {len(keyboard_fds)} evdev reader threads.")
        return True

    def _device_reader_loop(self, path: str, fd: int):
        while self.is_running:
            try:
                data = os.read(fd, EVENT_SIZE * 16)
                if not data:
                    break

                for i in range(0, len(data), EVENT_SIZE):
                    chunk = data[i : i + EVENT_SIZE]
                    if len(chunk) < EVENT_SIZE:
                        continue

                    _, _, ev_type, code, value = struct.unpack(EVENT_FORMAT, chunk)

                    if ev_type == EV_KEY:
                        key_name = LINUX_KEY_MAP.get(code, f"KEY_{code}")
                        if value == KEY_DOWN:
                            self.on_key_down(key_name)
                        elif value == KEY_UP:
                            self.on_key_up(key_name)
            except OSError:
                break
            except Exception as e:
                logger.debug(f"Error reading evdev event from {path}: {e}")
                time.sleep(0.01)

    def stop(self):
        self.is_running = False
        for fd in self._file_descriptors:
            try:
                os.close(fd)
            except Exception:
                pass
        self._file_descriptors.clear()
        self._threads.clear()


class HotkeyManager:
    """
    Unified 3-Tier Global Hotkey Manager for Audiover.

    Fallback Tiers:
      1. Tier 1: Modern Wayland XDG Desktop Portal (org.freedesktop.portal.GlobalShortcuts)
      2. Tier 2: Linux /dev/input raw character device listener (evdev)
      3. Tier 3: In-Window focused fallback via PyWebView / React event dispatching
    """

    def __init__(self):
        self.registered_hotkeys: Dict[str, Callable[[], None]] = {}
        self.is_running = False
        self.active_backend: HotkeyBackend = HotkeyBackend.IN_WINDOW
        self._pressed_keys: Set[str] = set()
        self._last_trigger_times: Dict[str, float] = {}
        self._debounce_interval: float = 0.15  # 150ms debounce threshold
        self._lock = threading.Lock()

        # Backend listeners
        self._portal_listener = XDGPortalHotkeyListener(on_trigger=self.trigger_hotkey_manually)
        self._evdev_listener = EvdevHotkeyListener(
            on_key_down=self._handle_evdev_key_down,
            on_key_up=self._handle_evdev_key_up,
        )

    def _should_trigger(self, key_str: str) -> bool:
        """Debounces rapid duplicate events within the debounce window."""
        now = time.monotonic()
        last = self._last_trigger_times.get(key_str, 0.0)
        if now - last < self._debounce_interval:
            return False
        self._last_trigger_times[key_str] = now
        return True

    @property
    def has_global_permissions(self) -> bool:
        """Returns True if a system-wide global listener is active."""
        return self.active_backend in (HotkeyBackend.PORTAL, HotkeyBackend.EVDEV)

    def check_permissions(self) -> bool:
        """Checks if either XDG Portal or /dev/input is available."""
        if self.active_backend == HotkeyBackend.PORTAL:
            return True
        return self._evdev_listener.check_permissions()

    def register_hotkey(self, hotkey_str: str, callback: Callable[[], None]):
        """Registers a callback for a given hotkey string (e.g. 'F8', 'F9', 'SPACE', '1')."""
        if not hotkey_str or not hotkey_str.strip():
            return
        clean_key = hotkey_str.strip().upper()
        with self._lock:
            self.registered_hotkeys[clean_key] = callback
            logger.info(f"Registered hotkey: {clean_key} (active backend: {self.active_backend.value})")

        # Sync with portal if active
        if self.active_backend == HotkeyBackend.PORTAL:
            with self._lock:
                keys = set(self.registered_hotkeys.keys())
            self._portal_listener.sync_shortcuts(keys)

    def unregister_hotkey(self, hotkey_str: str):
        if not hotkey_str:
            return
        clean_key = hotkey_str.strip().upper()
        with self._lock:
            if clean_key in self.registered_hotkeys:
                del self.registered_hotkeys[clean_key]

        if self.active_backend == HotkeyBackend.PORTAL:
            with self._lock:
                keys = set(self.registered_hotkeys.keys())
            self._portal_listener.sync_shortcuts(keys)

    def clear_hotkeys(self):
        with self._lock:
            self.registered_hotkeys.clear()
        if self.active_backend == HotkeyBackend.PORTAL:
            self._portal_listener.sync_shortcuts(set())

    def start(self):
        """Starts the highest available hotkey listener tier."""
        if self.is_running:
            return

        self.is_running = True

        # ── Tier 1: Try XDG Desktop Portal ──
        logger.info("Attempting Tier 1 (XDG Desktop Portal GlobalShortcuts)...")
        if self._portal_listener.start():
            self.active_backend = HotkeyBackend.PORTAL
            with self._lock:
                keys = set(self.registered_hotkeys.keys())
            self._portal_listener.sync_shortcuts(keys)
            logger.info("Tier 1 Active: Running via XDG Desktop Portal.")
            return

        # ── Tier 2: Try evdev /dev/input ──
        logger.info("Tier 1 unavailable. Attempting Tier 2 (/dev/input evdev)...")
        if self._evdev_listener.start():
            self.active_backend = HotkeyBackend.EVDEV
            logger.info("Tier 2 Active: Running via Linux /dev/input event listener.")
            return

        # ── Tier 3: In-Window Fallback ──
        self.active_backend = HotkeyBackend.IN_WINDOW
        logger.warning(
            "Tier 3 Fallback: Global input permissions unavailable. "
            "Hotkeys will function inside the Audiover application window."
        )

    def stop(self):
        """Stops all active listeners."""
        self.is_running = False
        self._portal_listener.stop()
        self._evdev_listener.stop()
        self.active_backend = HotkeyBackend.IN_WINDOW

    def _handle_evdev_key_down(self, key_name: str):
        with self._lock:
            if key_name in self._pressed_keys:
                return
            self._pressed_keys.add(key_name)
            active_combo = "+".join(sorted(list(self._pressed_keys)))
            callback = self.registered_hotkeys.get(key_name) or self.registered_hotkeys.get(active_combo)

            if callback and not self._should_trigger(key_name):
                return

        if callback:
            try:
                logger.info(f"Triggering hotkey action (evdev) for: {key_name}")
                callback()
            except Exception as e:
                logger.error(f"Error in hotkey callback: {e}")

    def _handle_evdev_key_up(self, key_name: str):
        with self._lock:
            self._pressed_keys.discard(key_name)

    def trigger_hotkey_manually(self, key_str: str):
        """
        Manually triggers the callback registered for a key string.
        Invoked by Portal listener, in-window webview keydown events, or Qt events.
        """
        if not key_str:
            return
        clean = key_str.strip().upper()
        callback = None
        with self._lock:
            if not self._should_trigger(clean):
                return
            callback = self.registered_hotkeys.get(clean)

        if callback:
            try:
                logger.info(f"Triggering hotkey action: {clean}")
                callback()
            except Exception as e:
                logger.error(f"Error executing hotkey callback for {clean}: {e}")

    def get_status(self) -> dict:
        """Returns comprehensive status dictionary for API and UI consumers."""
        return {
            "backend": self.active_backend.value,
            "has_permission": self.has_global_permissions,
            "is_running": self.is_running,
        }

