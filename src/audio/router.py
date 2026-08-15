import atexit
import logging
import re
import subprocess
from typing import Dict, List, Optional, Tuple

logger = logging.getLogger("Audiover.Router")


class AudioRouter:
    """Manages PipeWire / PulseAudio virtual audio sinks and sources for Audiover."""

    def __init__(
        self,
        sink_name: str = "Audiover_Sink",
        sink_desc: str = "Audiover_Virtual_Sink",
        source_name: str = "Audiover_Mic",
        source_desc: str = "Audiover_Virtual_Microphone",
    ):
        self.sink_name = sink_name
        self.sink_desc = sink_desc
        self.source_name = source_name
        self.source_desc = source_desc

        self.sink_module_id: Optional[str] = None
        self.source_module_id: Optional[str] = None
        self._is_setup = False

        # Register cleanup on program exit
        atexit.register(self.cleanup)

    def is_pipewire_available(self) -> bool:
        """Checks if PipeWire / PulseAudio daemon is responsive."""
        try:
            res = subprocess.run(
                ["pactl", "info"],
                capture_output=True,
                text=True,
                timeout=3,
                check=False,
            )
            return res.returncode == 0
        except Exception as e:
            logger.error(f"PipeWire check error: {e}")
            return False

    def setup_virtual_devices(self) -> bool:
        """Creates the virtual null-sink and remap-source on PipeWire."""
        # First ensure any lingering devices from a previous crash are removed
        self.remove_existing_devices()

        logger.info("Creating Audiover virtual audio devices...")
        try:
            # 1. Load module-null-sink
            sink_cmd = [
                "pactl",
                "load-module",
                "module-null-sink",
                f"sink_name={self.sink_name}",
                f'sink_properties=device.description="{self.sink_desc}"',
            ]
            res = subprocess.run(
                sink_cmd, capture_output=True, text=True, check=True
            )
            self.sink_module_id = res.stdout.strip()
            logger.info(
                f"Loaded Virtual Sink '{self.sink_name}' (Module ID: {self.sink_module_id})"
            )

            # 2. Load module-remap-source (virtual mic reading sink.monitor)
            source_cmd = [
                "pactl",
                "load-module",
                "module-remap-source",
                f"source_name={self.source_name}",
                f"master={self.sink_name}.monitor",
                f'source_properties=device.description="{self.source_desc}"',
            ]
            res2 = subprocess.run(
                source_cmd, capture_output=True, text=True, check=True
            )
            self.source_module_id = res2.stdout.strip()
            logger.info(
                f"Loaded Virtual Mic '{self.source_name}' (Module ID: {self.source_module_id})"
            )

            self._is_setup = True

            # Refresh sounddevice PortAudio cache once so the newly created virtual sink is discoverable
            try:
                import sounddevice as sd
                sd._terminate()
                sd._initialize()
            except Exception as e:
                logger.debug(f"sounddevice refresh: {e}")

            return True
        except Exception as e:
            logger.error(f"Failed to setup virtual audio devices: {e}")
            self.cleanup()
            return False

    def remove_existing_devices(self) -> None:
        """Checks for and removes any existing Audiover modules from pactl."""
        try:
            res = subprocess.run(
                ["pactl", "list", "short", "modules"],
                capture_output=True,
                text=True,
                timeout=3,
                check=False,
            )
            if res.returncode == 0:
                for line in res.stdout.splitlines():
                    if (
                        self.sink_name in line
                        or self.source_name in line
                        or "Audiover" in line
                    ):
                        parts = line.split()
                        if parts:
                            mod_id = parts[0]
                            logger.info(
                                f"Unloading existing Audiover module: {mod_id}"
                            )
                            subprocess.run(
                                ["pactl", "unload-module", mod_id],
                                capture_output=True,
                                timeout=2,
                                check=False,
                            )
        except Exception as e:
            logger.debug(f"Error checking existing modules: {e}")

    def cleanup(self) -> None:
        """Unloads the created virtual devices from PipeWire."""
        if self.source_module_id:
            try:
                subprocess.run(
                    ["pactl", "unload-module", self.source_module_id],
                    capture_output=True,
                    timeout=3,
                    check=False,
                )
                logger.info(
                    f"Unloaded virtual source module {self.source_module_id}"
                )
            except Exception as e:
                logger.debug(f"Error unloading source module: {e}")
            self.source_module_id = None

        if self.sink_module_id:
            try:
                subprocess.run(
                    ["pactl", "unload-module", self.sink_module_id],
                    capture_output=True,
                    timeout=3,
                    check=False,
                )
                logger.info(
                    f"Unloaded virtual sink module {self.sink_module_id}"
                )
            except Exception as e:
                logger.debug(f"Error unloading sink module: {e}")
            self.sink_module_id = None

        self._is_setup = False

    @staticmethod
    def get_audio_devices() -> (
        Tuple[List[Dict[str, any]], List[Dict[str, any]]]
    ):
        """Returns lists of available (input_devices, output_devices)."""
        import sounddevice as sd

        devices = sd.query_devices()
        inputs = []
        outputs = []

        for idx, dev in enumerate(devices):
            name = dev.get("name", f"Device {idx}")
            in_ch = dev.get("max_input_channels", 0)
            out_ch = dev.get("max_output_channels", 0)
            hostapi = dev.get("hostapi", 0)

            # Skip dummy or non-functional endpoints
            if in_ch > 0:
                inputs.append(
                    {
                        "index": idx,
                        "name": name,
                        "channels": in_ch,
                        "hostapi": hostapi,
                        "is_default": idx == sd.default.device[0],
                    }
                )
            if out_ch > 0:
                outputs.append(
                    {
                        "index": idx,
                        "name": name,
                        "channels": out_ch,
                        "hostapi": hostapi,
                        "is_default": idx == sd.default.device[1],
                    }
                )

        return inputs, outputs
