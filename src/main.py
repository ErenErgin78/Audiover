import os
import sys

# Ensure project root is in sys.path
PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if PROJECT_ROOT not in sys.path:
    sys.path.insert(0, PROJECT_ROOT)

import logging
import signal
import webview
from src.audio.dsp import VoiceDSP
from src.audio.router import AudioRouter
from src.audio.stream import AudioStreamEngine
from src.input.hotkeys import HotkeyManager
from src.soundboard.manager import SoundboardManager
from src.soundboard.player import SoundboardPlayer
from src.ui.api import AudioverAPI

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] (%(name)s) %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)

logger = logging.getLogger("Audiover.Main")

# Built React frontend path
UI_DIST = os.path.join(PROJECT_ROOT, "ui", "dist", "index.html")


def resolve_config_and_sounds_paths() -> tuple[str, str]:
    """Resolves XDG-compliant config and sound storage paths, seeding defaults on first run."""
    config_home = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    data_home = os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share")

    user_config_dir = os.path.join(config_home, "audiover")
    user_sounds_dir = os.path.join(data_home, "audiover", "sounds")
    user_config_path = os.path.join(user_config_dir, "settings.json")

    os.makedirs(user_config_dir, exist_ok=True)
    os.makedirs(user_sounds_dir, exist_ok=True)

    bundled_config = os.path.join(PROJECT_ROOT, "config", "settings.json")
    bundled_sounds = os.path.join(PROJECT_ROOT, "assets", "sounds")

    # Seed default sample sounds to user library
    if os.path.exists(bundled_sounds):
        import shutil
        for sound_file in os.listdir(bundled_sounds):
            src_file = os.path.join(bundled_sounds, sound_file)
            dst_file = os.path.join(user_sounds_dir, sound_file)
            if os.path.isfile(src_file) and not os.path.exists(dst_file):
                try:
                    shutil.copy2(src_file, dst_file)
                except Exception as e:
                    logger.debug(f"Could not copy default sound {sound_file}: {e}")

    # Seed default settings if not present
    if not os.path.exists(user_config_path) and os.path.exists(bundled_config):
        import shutil
        try:
            shutil.copy2(bundled_config, user_config_path)
        except Exception as e:
            logger.debug(f"Could not copy default settings: {e}")

    return user_config_path, user_sounds_dir


def main():
    logger.info("Initializing Audiover Engine...")

    # 1. Initialize Virtual Audio Router (PipeWire / PulseAudio)
    router = AudioRouter(
        sink_name="Audiover_Sink",
        sink_desc="Audiover_Virtual_Sink",
        source_name="Audiover_Mic",
        source_desc="Audiover_Virtual_Microphone",
    )

    if not router.setup_virtual_devices():
        logger.warning(
            "Could not setup PipeWire virtual devices automatically. "
            "Ensure pactl is installed and PipeWire is running."
        )

    # 2. Initialize DSP Engine
    sample_rate = 48000
    block_size = 256
    dsp = VoiceDSP(sample_rate=sample_rate, block_size=block_size)

    # 3. Initialize Soundboard Engine
    config_path, sounds_dir = resolve_config_and_sounds_paths()
    player = SoundboardPlayer(target_sample_rate=sample_rate)
    sb_manager = SoundboardManager(
        config_path=config_path,
        sounds_dir=sounds_dir,
        player=player,
    )
    sb_manager.load_from_config()

    # Read persistent audio & app settings if present
    saved_audio_cfg = {}
    if os.path.exists(config_path):
        try:
            import json
            with open(config_path, "r", encoding="utf-8") as f:
                settings_data = json.load(f)
                saved_audio_cfg = settings_data.get("audio", {})
        except Exception as e:
            logger.debug(f"Could not load initial audio settings: {e}")

    block_size = int(saved_audio_cfg.get("block_size", block_size))

    # 4. Initialize Real-Time Audio Streaming Engine
    stream_engine = AudioStreamEngine(
        dsp=dsp,
        soundboard_player=player,
        sample_rate=sample_rate,
        block_size=block_size,
    )

    # Apply saved audio configurations
    stream_engine.mic_gain = float(saved_audio_cfg.get("mic_gain", 1.0))
    stream_engine.monitor_gain = float(saved_audio_cfg.get("monitor_gain", 1.0))
    stream_engine.hear_myself = bool(saved_audio_cfg.get("hear_myself", False))
    stream_engine.hear_soundboard = bool(saved_audio_cfg.get("hear_soundboard", True))
    stream_engine.input_device = stream_engine.resolve_input_device(saved_audio_cfg.get("input_device_name"))
    stream_engine.monitor_device = stream_engine.resolve_monitor_device(saved_audio_cfg.get("monitor_device_name"))

    # 5. Initialize Global Hotkey System
    hotkey_mgr = HotkeyManager()

    # 6. Register global hotkeys
    hotkey_mgr.register_hotkey("F9", lambda: setattr(stream_engine, "is_muted", not stream_engine.is_muted))
    hotkey_mgr.register_hotkey("F10", _make_bypass_toggle(dsp))
    hotkey_mgr.register_hotkey("F11", player.stop_all)
    hotkey_mgr.register_hotkey("F8", lambda: stream_engine.set_hear_myself(not stream_engine.hear_myself))

    # 7. Start Audio Stream Engine & Hotkey Listeners
    stream_engine.start()
    hotkey_mgr.start()

    # 8. Create JS ↔ Python API bridge
    api = AudioverAPI(
        router=router,
        stream_engine=stream_engine,
        soundboard_player=player,
        soundboard_manager=sb_manager,
        hotkey_manager=hotkey_mgr,
    )

    # 9. Launch PyWebView window
    window = webview.create_window(
        title="Audiover — Voice & Soundboard Engine",
        url=UI_DIST,
        js_api=api,
        width=1080,
        height=720,
        min_size=(900, 600),
        background_color="#0F111A",
    )

    def on_closing():
        api.shutdown()

    window.events.closing += on_closing
    window.events.closed += on_closing

    # Handle Ctrl+C gracefully
    def sigint_handler(*_):
        logger.info("Received interrupt signal. Closing...")
        api.shutdown()
        os._exit(0)

    signal.signal(signal.SIGINT, sigint_handler)
    signal.signal(signal.SIGTERM, sigint_handler)

    try:
        webview.start(gui="qt", debug=False)
    finally:
        logger.info("Audiover closed gracefully.")
        api.shutdown()
        os._exit(0)


def _make_bypass_toggle(dsp: VoiceDSP):
    """F10 hotkey: toggle DSP bypass."""
    def toggle():
        opts = dsp.options  # mevcut DSPOptions'ı al
        opts.bypass = not opts.bypass
        dsp.update_options(opts)
    return toggle


if __name__ == "__main__":
    main()
