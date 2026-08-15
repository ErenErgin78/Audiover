import os
import sys

# Ensure project root is in sys.path
PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if PROJECT_ROOT not in sys.path:
    sys.path.insert(0, PROJECT_ROOT)

import logging
import signal
from PyQt6.QtWidgets import QApplication
from src.audio.dsp import VoiceDSP
from src.audio.router import AudioRouter
from src.audio.stream import AudioStreamEngine
from src.input.hotkeys import HotkeyManager
from src.soundboard.manager import SoundboardManager
from src.soundboard.player import SoundboardPlayer
from src.ui.main_window import MainWindow
from src.ui.styles import MODERN_STYLE

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] (%(name)s) %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)

logger = logging.getLogger("Audiover.Main")


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
            "Could not setup PipeWire virtual devices automatically. Ensure pactl is installed and PipeWire is running."
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

    # 6. Start PyQt6 GUI Application
    app = QApplication(sys.argv)
    app.setStyleSheet(MODERN_STYLE)

    window = MainWindow(
        router=router,
        dsp=dsp,
        stream_engine=stream_engine,
        soundboard_player=player,
        soundboard_manager=sb_manager,
        hotkey_manager=hotkey_mgr,
    )

    # 7. Start Audio Stream Engine after devices & panels are loaded
    stream_engine.start()

    window.show()

    # Handle Ctrl+C gracefully
    def sigint_handler(*_):
        logger.info("Received interrupt signal. Closing window...")
        window.close()
        app.quit()

    signal.signal(signal.SIGINT, sigint_handler)
    signal.signal(signal.SIGTERM, sigint_handler)

    exit_code = app.exec()

    # Final cleanup
    stream_engine.stop()
    hotkey_mgr.stop()
    router.cleanup()
    logger.info("Audiover closed gracefully.")
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
