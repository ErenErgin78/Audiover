"""
PyWebView JS ↔ Python API Bridge for Audiover.

All public methods are exposed to the frontend via window.pywebview.api.<method>().
"""

import base64
import json
import logging
import os
import shutil
import subprocess
from typing import Any, Optional
import webview

from ..audio.dsp import DSPOptions
from ..audio.router import AudioRouter
from ..audio.stream import AudioStreamEngine
from ..input.hotkeys import HotkeyManager
from ..soundboard.manager import SoundboardManager
from ..soundboard.player import SoundboardPlayer

logger = logging.getLogger("Audiover.API")


def pick_file_native(lang: str = "tr") -> tuple[Optional[str], bool]:
    """
    Opens native desktop file dialog (Zenity, Kdialog, etc.).
    Returns: (file_path, was_handled)
    """
    title = "Dosya Seç" if lang == "tr" else "Select Audio File"
    filter_label = "Ses ve Video Dosyaları" if lang == "tr" else "Audio and Video Files"
    all_label = "Tüm Dosyalar" if lang == "tr" else "All Files"

    if shutil.which("zenity"):
        try:
            cmd = [
                "zenity",
                "--file-selection",
                f"--title={title}",
                f"--file-filter={filter_label} | *.mp3 *.wav *.ogg *.flac *.m4a *.aac *.mp4 *.mkv *.mov *.webm *.opus",
                f"--file-filter={all_label} | *",
            ]
            proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            if proc.returncode == 0 and proc.stdout.strip():
                return proc.stdout.strip(), True
            return None, True
        except Exception as e:
            logger.debug(f"Zenity file picker error: {e}")

    if shutil.which("kdialog"):
        try:
            cmd = [
                "kdialog",
                "--getopenfilename",
                os.path.expanduser("~"),
                f"*.mp3 *.wav *.ogg *.flac *.m4a *.aac *.mp4 *.mkv *.mov *.webm *.opus|{filter_label}\n*|{all_label}",
                "--title",
                title,
            ]
            proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            if proc.returncode == 0 and proc.stdout.strip():
                return proc.stdout.strip(), True
            return None, True
        except Exception as e:
            logger.debug(f"Kdialog file picker error: {e}")

    return None, False


DEFAULT_PRESETS: dict[str, dict] = {
    "Clean": {
        "pitch": 0.0, "robot": False, "rfreq": 150, "rmix": 0.0,
        "radio": False, "dist": False, "drive": 0.0,
        "rev": False, "rsize": 0.0, "rwet": 0.0,
        "chorus": False, "cdepth": 0.0,
        "bypass": False, "gate": False, "gate_db": -65.0,
    },
    "Deep Voice": {
        "pitch": -5.5, "robot": False, "rfreq": 120, "rmix": 0.0,
        "radio": False, "dist": True, "drive": 0.20,
        "rev": True, "rsize": 0.40, "rwet": 0.25,
        "chorus": False, "cdepth": 0.0,
        "bypass": False, "gate": False, "gate_db": -65.0,
    },
}


class AudioverAPI:
    """
    JS ↔ Python API Bridge for PyWebView.
    """

    def __init__(
        self,
        router: AudioRouter,
        stream_engine: AudioStreamEngine,
        soundboard_player: SoundboardPlayer,
        soundboard_manager: SoundboardManager,
        hotkey_manager: HotkeyManager,
    ):
        self._router = router
        self._engine = stream_engine
        self._player = soundboard_player
        self._sb_manager = soundboard_manager
        self._hotkey_mgr = hotkey_manager
        self._config_path = soundboard_manager.config_path
        self._custom_presets: dict[str, dict] = {}
        self._active_preset: str = "Clean"
        self._language: str = "tr"
        self._audio_settings: dict = {}

        self._load_all_settings()
        self._sync_sound_hotkeys()

        # Apply active preset
        presets = self._all_presets()
        if self._active_preset in presets:
            self._apply_dsp_config(presets[self._active_preset])

    # ------------------------------------------------------------------
    # Settings Management
    # ------------------------------------------------------------------

    def _load_all_settings(self) -> None:
        if not os.path.exists(self._config_path):
            return
        try:
            with open(self._config_path, "r", encoding="utf-8") as f:
                settings = json.load(f)

            # 1. App settings
            app_data = settings.get("app", {})
            self._language = app_data.get("language", "tr")

            # 2. Audio settings
            self._audio_settings = settings.get("audio", {})

            # 3. Voice effects
            voice_data = settings.get("voice_effects", {})
            self._custom_presets = voice_data.get("custom_presets", {})
            saved_active = voice_data.get("active_preset")
            if saved_active and (saved_active in DEFAULT_PRESETS or saved_active in self._custom_presets):
                self._active_preset = saved_active
        except Exception as e:
            logger.error(f"Error loading configuration: {e}")

    def _save_settings(self) -> None:
        try:
            settings = {}
            if os.path.exists(self._config_path):
                try:
                    with open(self._config_path, "r", encoding="utf-8") as f:
                        settings = json.load(f)
                except Exception:
                    settings = {}

            # Update App section
            app_data = settings.setdefault("app", {})
            app_data["language"] = self._language

            # Update Audio section
            settings["audio"] = self._audio_settings

            # Update Voice Effects section
            voice_data = settings.setdefault("voice_effects", {})
            voice_data["custom_presets"] = self._custom_presets
            voice_data["active_preset"] = self._active_preset

            os.makedirs(os.path.dirname(self._config_path), exist_ok=True)
            with open(self._config_path, "w", encoding="utf-8") as f:
                json.dump(settings, f, indent=2, ensure_ascii=False)
        except Exception as e:
            logger.error(f"Failed to persist settings: {e}")

    # ------------------------------------------------------------------
    # State Snapshot
    # ------------------------------------------------------------------

    def get_state(self) -> dict:
        """Retrieves full application snapshot for initial frontend mount."""
        return {
            "engine_active": self._engine.is_running,
            "is_muted": self._engine.is_muted,
            "hear_myself": self._engine.hear_myself,
            "hear_soundboard": self._engine.hear_soundboard,
            "mic_gain": self._engine.mic_gain,
            "monitor_gain": self._engine.monitor_gain,
            "active_preset": self._active_preset,
            "presets": self._all_presets(),
            "hotkey_permission": self._hotkey_mgr.check_permissions(),
            "language": self._language,
        }

    # ------------------------------------------------------------------
    # App Settings / Localization
    # ------------------------------------------------------------------

    def set_language(self, language: str) -> dict:
        """Changes the interface language ('tr' | 'en') and persists it."""
        if language in ["tr", "en"]:
            self._language = language
            self._save_settings()
            return {"ok": True, "language": self._language}
        return {"ok": False, "error": "Unsupported language"}

    # ------------------------------------------------------------------
    # Engine Controls
    # ------------------------------------------------------------------

    def set_engine_active(self, active: bool) -> dict:
        if active:
            success = self._engine.start()
            return {"ok": success, "active": success}
        else:
            self._engine.stop()
            return {"ok": True, "active": False}

    def set_muted(self, muted: bool) -> None:
        self._engine.is_muted = muted

    def set_hear_myself(self, enabled: bool) -> None:
        self._engine.set_hear_myself(enabled)
        self._audio_settings["hear_myself"] = enabled
        self._save_settings()

    def set_hear_soundboard(self, enabled: bool) -> None:
        self._engine.hear_soundboard = enabled
        self._audio_settings["hear_soundboard"] = enabled
        self._save_settings()

    # ------------------------------------------------------------------
    # VU Meters (polling)
    # ------------------------------------------------------------------

    def get_meters(self) -> dict:
        return {
            "in_peak": round(self._engine.meter_input_peak, 4),
            "in_rms": round(self._engine.meter_input_rms, 4),
            "out_peak": round(self._engine.meter_output_peak, 4),
            "out_rms": round(self._engine.meter_output_rms, 4),
        }

    # ------------------------------------------------------------------
    # DSP / Voice Presets
    # ------------------------------------------------------------------

    def get_presets(self) -> dict:
        return {"presets": self._all_presets(), "active": self._active_preset}

    def apply_preset(self, name: str) -> dict:
        presets = self._all_presets()
        if name not in presets:
            return {"ok": False, "error": f"Preset '{name}' not found"}
        cfg = presets[name]
        self._active_preset = name
        self._apply_dsp_config(cfg)
        self._save_settings()
        return {"ok": True, "active": name}

    def update_dsp(self, opts: dict) -> None:
        self._apply_dsp_config(opts)
        if self._active_preset:
            self._custom_presets[self._active_preset] = opts
            self._save_settings()

    def reset_preset(self, name: str) -> dict:
        if name not in DEFAULT_PRESETS:
            return {"ok": False, "error": "Only default presets can be reset"}
        if name in self._custom_presets:
            del self._custom_presets[name]
            self._save_settings()
        cfg = DEFAULT_PRESETS[name]
        if self._active_preset == name:
            self._apply_dsp_config(cfg)
        return {"ok": True, "presets": self._all_presets(), "config": cfg}

    def create_preset(self, name: str, config: dict) -> dict:
        name = name.strip()
        if not name:
            return {"ok": False, "error": "Name cannot be empty"}
        if name in DEFAULT_PRESETS:
            return {"ok": False, "error": "Cannot overwrite built-in preset"}
        self._custom_presets[name] = config
        self._active_preset = name
        self._save_settings()
        return {"ok": True, "name": name, "presets": self._all_presets()}

    def save_preset(self, name: str, config: dict) -> dict:
        name = name.strip()
        if not name:
            return {"ok": False, "error": "Name cannot be empty"}
        self._custom_presets[name] = config
        self._save_settings()
        return {"ok": True, "presets": self._all_presets()}

    def delete_preset(self, name: str) -> dict:
        if name in DEFAULT_PRESETS:
            return {"ok": False, "error": "Cannot delete built-in preset"}
        if name not in self._custom_presets:
            return {"ok": False, "error": "Preset not found"}
        del self._custom_presets[name]
        if self._active_preset == name:
            self._active_preset = "Clean"
            presets = self._all_presets()
            self._apply_dsp_config(presets.get("Clean", DEFAULT_PRESETS["Clean"]))
        self._save_settings()
        return {"ok": True, "presets": self._all_presets(), "active": self._active_preset}

    # ------------------------------------------------------------------
    # Soundboard
    # ------------------------------------------------------------------

    def get_sounds(self) -> list:
        return [self._sound_to_dict(s) for s in self._sb_manager.get_all_sounds()]

    def add_sound_file(self) -> dict:
        file_path, was_handled = pick_file_native(lang=self._language)
        if was_handled:
            if not file_path or not os.path.exists(file_path):
                return {"ok": False, "cancelled": True}
        else:
            win = webview.windows[0] if webview.windows else None
            if win is None:
                return {"ok": False, "cancelled": True}
            result = win.create_file_dialog(
                webview.OPEN_DIALOG,
                allow_multiple=False,
                file_types=(
                    "Audio and Video Files (*.mp4;*.mp3;*.wav;*.ogg;*.flac;*.m4a;*.aac;*.mkv;*.mov)",
                    "All Files (*.*)",
                ),
            )
            if not result or not result[0]:
                return {"ok": False, "cancelled": True}
            file_path = result[0]

        if not file_path or not os.path.exists(file_path):
            return {"ok": False, "cancelled": True}

        item = self._sb_manager.add_sound_file(file_path=file_path, copy_to_assets=True)
        if item:
            return {"ok": True, "sound": self._sound_to_dict(item)}
        return {"ok": False, "error": f"Could not decode: {os.path.basename(file_path)}"}

    def add_sound_data(self, filename: str, base64_data: str) -> dict:
        try:
            if "," in base64_data:
                base64_data = base64_data.split(",", 1)[1]
            raw_bytes = base64.b64decode(base64_data)
            item = self._sb_manager.add_sound_from_bytes(filename=filename, data_bytes=raw_bytes)
            if item:
                return {"ok": True, "sound": self._sound_to_dict(item)}
            return {"ok": False, "error": f"Could not decode: {filename}"}
        except Exception as e:
            logger.error(f"Error saving dropped sound file: {e}")
            return {"ok": False, "error": str(e)}

    def play_sound(self, sound_id: str) -> None:
        self._player.play(sound_id, restart=True)

    def pause_sound(self, sound_id: str) -> None:
        self._player.pause(sound_id)

    def stop_sound(self, sound_id: str) -> None:
        self._player.stop(sound_id)

    def stop_all_sounds(self) -> None:
        self._player.stop_all()

    def get_sound_progress(self, sound_id: str) -> float:
        return self._player.get_progress(sound_id)

    def get_all_progress(self) -> dict:
        result = {}
        for sound_id, track in self._player.tracks.items():
            result[sound_id] = {
                "is_playing": track.is_playing,
                "progress": self._player.get_progress(sound_id),
            }
        return result

    def update_sound(self, sound_id: str, volume: Optional[float] = None,
                     loop: Optional[bool] = None, hotkey: Optional[str] = None) -> dict:
        kwargs = {}
        if volume is not None:
            kwargs["volume"] = volume
        if loop is not None:
            kwargs["loop"] = loop
        if hotkey is not None:
            clean_hk = hotkey.strip().upper()
            kwargs["hotkey"] = clean_hk if clean_hk else ""
            old_item = self._sb_manager.get_sound(sound_id)
            if old_item and old_item.hotkey:
                self._hotkey_mgr.unregister_hotkey(old_item.hotkey)
            if clean_hk:
                self._hotkey_mgr.register_hotkey(
                    clean_hk,
                    lambda s_id=sound_id: self._player.play(s_id, restart=True),
                )
        self._sb_manager.update_sound(sound_id, **kwargs)
        return {"ok": True}

    def remove_sound(self, sound_id: str) -> dict:
        old_item = self._sb_manager.get_sound(sound_id)
        if old_item and old_item.hotkey:
            self._hotkey_mgr.unregister_hotkey(old_item.hotkey)
        self._sb_manager.remove_sound(sound_id)
        return {"ok": True}

    def _sync_sound_hotkeys(self) -> None:
        for sound in self._sb_manager.get_all_sounds():
            if sound.hotkey and sound.hotkey.strip():
                clean_hk = sound.hotkey.strip().upper()
                self._hotkey_mgr.register_hotkey(
                    clean_hk,
                    lambda s_id=sound.id: self._player.play(s_id, restart=True),
                )

    # ------------------------------------------------------------------
    # Audio Settings & Hardware Persistence
    # ------------------------------------------------------------------

    def get_audio_devices(self) -> dict:
        inputs, outputs = self._router.get_audio_devices()
        # Filter out Audiover internal virtual devices
        _skip = {"audiover", "virtual_sink", "null_sink", "remap_source"}
        filtered_inputs = [
            d for d in inputs
            if not any(k in d["name"].lower() for k in _skip)
        ]
        filtered_outputs = [
            d for d in outputs
            if not any(k in d["name"].lower() for k in _skip)
        ]
        return {
            "inputs": filtered_inputs,
            "outputs": filtered_outputs,
            "current_input": self._engine.input_device,
            "current_monitor": self._engine.monitor_device,
            "block_size": self._engine.block_size,
            "mic_gain": self._engine.mic_gain,
            "monitor_gain": self._engine.monitor_gain,
            "hear_myself": self._engine.hear_myself,
            "hear_soundboard": self._engine.hear_soundboard,
        }

    def set_input_device(self, index: int) -> None:
        self._engine.input_device = index
        dev_name = self._engine.get_device_name(index)
        self._audio_settings["input_device_name"] = dev_name
        self._save_settings()
        if self._engine.is_running:
            self._engine.restart()

    def set_monitor_device(self, index: Optional[int]) -> None:
        self._engine.monitor_device = index
        dev_name = self._engine.get_device_name(index) if index is not None else "none"
        self._audio_settings["monitor_device_name"] = dev_name
        self._save_settings()
        if self._engine.is_running:
            self._engine.restart()

    def set_buffer_size(self, size: int) -> None:
        self._engine.block_size = size
        self._audio_settings["block_size"] = size
        self._save_settings()
        if self._engine.is_running:
            self._engine.restart()

    def set_mic_gain(self, gain: float) -> None:
        self._engine.mic_gain = gain
        self._audio_settings["mic_gain"] = gain
        self._save_settings()

    def set_monitor_gain(self, gain: float) -> None:
        self._engine.monitor_gain = gain
        self._audio_settings["monitor_gain"] = gain
        self._save_settings()

    # ------------------------------------------------------------------
    # Hotkeys
    # ------------------------------------------------------------------

    def get_hotkey_status(self) -> dict:
        has_perm = self._hotkey_mgr.check_permissions()
        return {
            "has_permission": has_perm,
            "hotkeys": [
                {"action": "Mute Microphone", "key": "F9"},
                {"action": "Bypass All DSP Effects", "key": "F10"},
                {"action": "Stop All Sounds (Panic)", "key": "F11"},
                {"action": "Toggle Hear Myself (Loopback)", "key": "F8"},
            ],
        }

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def shutdown(self) -> None:
        if getattr(self, "_is_shutdown", False):
            return
        self._is_shutdown = True
        logger.info("Shutting down Audiover via API...")
        try:
            self._engine.stop()
        except Exception as e:
            logger.debug(f"Error stopping engine: {e}")
        try:
            self._hotkey_mgr.stop()
        except Exception as e:
            logger.debug(f"Error stopping hotkeys: {e}")
        try:
            self._player.stop_all()
        except Exception as e:
            logger.debug(f"Error stopping player: {e}")
        try:
            self._sb_manager.save_to_config()
        except Exception as e:
            logger.debug(f"Error saving config: {e}")
        try:
            self._save_settings()
        except Exception as e:
            logger.debug(f"Error saving all settings: {e}")
        try:
            self._router.cleanup()
        except Exception as e:
            logger.debug(f"Error cleaning router: {e}")

    # ------------------------------------------------------------------
    # Internal Helpers
    # ------------------------------------------------------------------

    def _all_presets(self) -> dict:
        return {**DEFAULT_PRESETS, **self._custom_presets}

    def _apply_dsp_config(self, cfg: dict) -> None:
        opts = DSPOptions(
            bypass=cfg.get("bypass", False),
            noise_gate_enabled=cfg.get("gate", False),
            noise_gate_threshold_db=float(cfg.get("gate_db", -65.0)),
            pitch_semitones=float(cfg.get("pitch", 0.0)),
            robot_enabled=cfg.get("robot", False),
            robot_freq=float(cfg.get("rfreq", 150.0)),
            robot_mix=float(cfg.get("rmix", 0.0)),
            radio_enabled=cfg.get("radio", False),
            distortion_enabled=cfg.get("dist", False),
            distortion_drive=float(cfg.get("drive", 0.0)),
            reverb_enabled=cfg.get("rev", False),
            reverb_room_size=float(cfg.get("rsize", 0.6)),
            reverb_wet=float(cfg.get("rwet", 0.0)),
            chorus_enabled=cfg.get("chorus", False),
            chorus_depth=float(cfg.get("cdepth", 0.0)),
        )
        self._engine.dsp.update_options(opts)

    def _sound_to_dict(self, item: Any) -> dict:
        return {
            "id": item.id,
            "name": item.name,
            "file_path": item.file_path,
            "volume": item.volume,
            "loop": item.loop,
            "hotkey": item.hotkey or "",
        }
