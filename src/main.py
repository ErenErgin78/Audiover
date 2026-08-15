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
    player = SoundboardPlayer(target_sample_rate=sample_rate)
    sb_manager = SoundboardManager(
        config_path=os.path.join(PROJECT_ROOT, "config", "settings.json"),
        sounds_dir=os.path.join(PROJECT_ROOT, "assets", "sounds"),
        player=player,
    )
    sb_manager.load_from_config()

    # 4. Initialize Real-Time Audio Streaming Engine
    stream_engine = AudioStreamEngine(
        dsp=dsp,
        soundboard_player=player,
        sample_rate=sample_rate,
        block_size=block_size,
    )

    # 5. Initialize Global Hotkey System
    hotkey_mgr = HotkeyManager()
    hotkey_mgr.start()

    # 6. Register global hotkeys
    hotkey_mgr.register_hotkey("F9", lambda: setattr(stream_engine, "is_muted", not stream_engine.is_muted))
    hotkey_mgr.register_hotkey("F10", _make_bypass_toggle(dsp))
    hotkey_mgr.register_hotkey("F11", player.stop_all)
    hotkey_mgr.register_hotkey("F8", lambda: stream_engine.set_hear_myself(not stream_engine.hear_myself))

    # 7. Start Audio Stream Engine
    stream_engine.start()

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
