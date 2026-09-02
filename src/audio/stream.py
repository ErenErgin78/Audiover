import logging
import queue
import threading
import time
from typing import Callable, Optional, Tuple
import numpy as np
import sounddevice as sd
from .dsp import VoiceDSP
from ..soundboard.player import SoundboardPlayer

logger = logging.getLogger("Audiover.StreamEngine")


class AudioStreamEngine:
    """Real-Time Low-Latency Audio Stream Routing and Mixing Engine."""

    def __init__(
        self,
        dsp: VoiceDSP,
        soundboard_player: SoundboardPlayer,
        sample_rate: int = 48000,
        block_size: int = 256,
    ):
        self.dsp = dsp
        self.soundboard = soundboard_player
        self.sample_rate = sample_rate
        self.block_size = block_size

        # Device selections (index)
        self.input_device: Optional[int] = None
        self.virtual_sink_device: Optional[int] = None
        self.monitor_device: Optional[int] = None

        # Control States
        self.is_running = False
        self.is_muted = False
        self.hear_myself = False  # Voice Loopback
        self.hear_soundboard = True  # Soundboard to headphones

        # Gains
        self.mic_gain = 1.0
        self.soundboard_gain = 1.0
        self.monitor_gain = 1.0

        # Peak & RMS Meters (values in [0.0, 1.0])
        self.meter_input_peak = 0.0
        self.meter_input_rms = 0.0
        self.meter_output_peak = 0.0
        self.meter_output_rms = 0.0

        # Stream Handles
        self._in_stream: Optional[sd.InputStream] = None
        self._virt_out_stream: Optional[sd.OutputStream] = None
        self._mon_out_stream: Optional[sd.OutputStream] = None

        # Thread-safe queues for cross-stream output
        self._virt_queue = queue.Queue(maxsize=32)
        self._mon_queue = queue.Queue(maxsize=32)

        # Lock for safe configuration changes
        self._lock = threading.Lock()

        # Meter callback
        self.on_meter_update: Optional[
            Callable[[float, float, float, float], None]
        ] = None

    @staticmethod
    def get_device_name(device_index: Optional[int]) -> Optional[str]:
        """Gets device name from its index."""
        if device_index is None:
            return None
        try:
            dev = sd.query_devices(device_index)
            return dev.get("name")
        except Exception:
            return None

    def find_virtual_sink_index(
        self,
        sink_name: str = "Audiover_Sink",
        sink_desc: str = "Audiover_Virtual_Sink",
    ) -> Optional[int]:
        """Finds the sounddevice device index corresponding to the virtual null sink."""
        try:
            devices = sd.query_devices()
        except Exception as e:
            logger.error(f"Error querying audio devices: {e}")
            return None

        # 1. Primary search: exact or description match on Sink, excluding Mic / Source
        for idx, dev in enumerate(devices):
            d_name = dev.get("name", "")
            d_lower = d_name.lower()
            if (
                (
                    sink_name.lower() in d_lower
                    or sink_desc.lower() in d_lower
                    or "audiover_sink" in d_lower
                )
                and "mic" not in d_lower
                and "source" not in d_lower
                and dev.get("max_output_channels", 0) > 0
            ):
                logger.info(
                    f"Found Virtual Sink device: [{idx}] {d_name} (out: {dev.get('max_output_channels')})"
                )
                return idx

        # 2. Fallback search: any Audiover device with 'sink' in name
        for idx, dev in enumerate(devices):
            d_name = dev.get("name", "")
            d_lower = d_name.lower()
            if (
                "audiover" in d_lower
                and "sink" in d_lower
                and "mic" not in d_lower
                and dev.get("max_output_channels", 0) > 0
            ):
                logger.info(
                    f"Fallback matched Virtual Sink device: [{idx}] {d_name}"
                )
                return idx

        logger.warning("Audiover virtual sink device not detected in sounddevice.")
        return None

    def resolve_input_device(self, saved_name: Optional[str] = None) -> Optional[int]:
        """
        Resolves input device:
        1. Matches by saved_name if provided and exists.
        2. Fallbacks to sounddevice system default input device.
        3. Fallbacks to the first non-virtual input device.
        """
        try:
            devices = sd.query_devices()
        except Exception as e:
            logger.error(f"Error querying devices for input: {e}")
            return None

        # 1. Saved device name lookup
        if saved_name:
            clean_saved = saved_name.strip().lower()
            for idx, dev in enumerate(devices):
                if dev.get("max_input_channels", 0) > 0:
                    d_name = dev.get("name", "").lower()
                    if clean_saved == d_name or clean_saved in d_name:
                        if "audiover" not in d_name:
                            logger.info(
                                f"Restored saved input device: [{idx}] {dev.get('name')}"
                            )
                            return idx

        # 2. System default input device
        try:
            default_in = sd.default.device[0]
            if 0 <= default_in < len(devices):
                dev = devices[default_in]
                d_name = dev.get("name", "").lower()
                if dev.get("max_input_channels", 0) > 0 and "audiover" not in d_name:
                    logger.info(
                        f"Using system default input device: [{default_in}] {dev.get('name')}"
                    )
                    return default_in
        except Exception as e:
            logger.debug(f"Default input device query failed: {e}")

        # 3. First non-virtual input device
        for idx, dev in enumerate(devices):
            if dev.get("max_input_channels", 0) > 0:
                d_name = dev.get("name", "").lower()
                if "audiover" not in d_name:
                    logger.info(
                        f"Fallback selected input device: [{idx}] {dev.get('name')}"
                    )
                    return idx

        return None

    def resolve_monitor_device(self, saved_name: Optional[str] = None) -> Optional[int]:
        """
        Resolves monitor (headphones / speaker) output device:
        1. If saved_name is explicitly "none" or disabled, returns None.
        2. Matches by saved_name if provided.
        3. Fallbacks to sounddevice system default output device.
        4. Fallbacks to the first non-virtual output device.
        """
        if saved_name == "none":
            return None

        try:
            devices = sd.query_devices()
        except Exception as e:
            logger.error(f"Error querying devices for monitor: {e}")
            return None

        # 1. Saved device name lookup
        if saved_name:
            clean_saved = saved_name.strip().lower()
            for idx, dev in enumerate(devices):
                if dev.get("max_output_channels", 0) > 0:
                    d_name = dev.get("name", "").lower()
                    if clean_saved == d_name or clean_saved in d_name:
                        if "audiover" not in d_name:
                            logger.info(
                                f"Restored saved monitor device: [{idx}] {dev.get('name')}"
                            )
                            return idx

        # 2. System default output device
        try:
            default_out = sd.default.device[1]
            if 0 <= default_out < len(devices):
                dev = devices[default_out]
                d_name = dev.get("name", "").lower()
                if dev.get("max_output_channels", 0) > 0 and "audiover" not in d_name:
                    logger.info(
                        f"Using system default output device: [{default_out}] {dev.get('name')}"
                    )
                    return default_out
        except Exception as e:
            logger.debug(f"Default output device query failed: {e}")

        # 3. First non-virtual output device
        for idx, dev in enumerate(devices):
            if dev.get("max_output_channels", 0) > 0:
                d_name = dev.get("name", "").lower()
                if "audiover" not in d_name:
                    logger.info(
                        f"Fallback selected monitor device: [{idx}] {dev.get('name')}"
                    )
                    return idx

        return None

    def set_hear_myself(self, enabled: bool):
        """Sets loopback monitoring state and ensures monitor stream is active."""
        with self._lock:
            self.hear_myself = enabled
            if enabled and self.is_running:
                # Pre-buffer 2 silent blocks so monitor stream never starves
                while not self._mon_queue.empty():
                    try:
                        self._mon_queue.get_nowait()
                    except queue.Empty:
                        break
                silence = np.zeros((self.block_size, 2), dtype=np.float32)
                for _ in range(2):
                    self._mon_queue.put_nowait(silence)

                if self._mon_out_stream is None:
                    if self.monitor_device is None:
                        self.monitor_device = self.resolve_monitor_device()
                    if self.monitor_device is not None:
                        try:
                            self._mon_out_stream = sd.OutputStream(
                                device=self.monitor_device,
                                channels=2,
                                samplerate=self.sample_rate,
                                blocksize=self.block_size,
                                dtype="float32",
                                latency="low",
                                callback=self._mon_out_callback,
                            )
                            self._mon_out_stream.start()
                            logger.info(
                                f"Started Monitor Output Stream on device {self.monitor_device}"
                            )
                        except Exception as e:
                            logger.warning(
                                f"Could not open monitor device {self.monitor_device}: {e}"
                            )

    def start(self) -> bool:
        """Starts audio capture, DSP processing, and output streaming."""
        with self._lock:
            if self.is_running:
                return True

            logger.info("Starting AudioStreamEngine...")
            try:
                # Find Virtual Sink if not set
                if self.virtual_sink_device is None:
                    self.virtual_sink_device = self.find_virtual_sink_index()
                    if self.virtual_sink_device is None:
                        # Give PipeWire a brief moment if devices were just created
                        time.sleep(0.15)
                        self.virtual_sink_device = (
                            self.find_virtual_sink_index()
                        )

                # Ensure physical input device is explicitly resolved
                if self.input_device is None:
                    self.input_device = self.resolve_input_device()

                # Ensure monitor output device is resolved
                if self.monitor_device is None:
                    self.monitor_device = self.resolve_monitor_device()

                # Clear and pre-buffer queues with 2 blocks of silence
                while not self._virt_queue.empty():
                    self._virt_queue.get_nowait()
                while not self._mon_queue.empty():
                    self._mon_queue.get_nowait()

                silence = np.zeros((self.block_size, 2), dtype=np.float32)
                for _ in range(2):
                    self._virt_queue.put_nowait(silence)
                    self._mon_queue.put_nowait(silence)

                # 1. Virtual Sink Output Stream (Feeds Virtual Mic)
                if self.virtual_sink_device is not None:
                    try:
                        self._virt_out_stream = sd.OutputStream(
                            device=self.virtual_sink_device,
                            channels=2,
                            samplerate=self.sample_rate,
                            blocksize=self.block_size,
                            dtype="float32",
                            latency="low",
                            callback=self._virt_out_callback,
                        )
                        self._virt_out_stream.start()
                        logger.info(
                            f"Started Virtual Sink Output Stream on device {self.virtual_sink_device}"
                        )
                    except Exception as e:
                        logger.warning(
                            f"Could not open virtual sink stream on device {self.virtual_sink_device}: {e}"
                        )
                else:
                    logger.warning(
                        "No virtual sink device found! Output won't reach virtual mic."
                    )

                # 2. Monitor Headphone Output Stream
                if self.monitor_device is not None:
                    try:
                        self._mon_out_stream = sd.OutputStream(
                            device=self.monitor_device,
                            channels=2,
                            samplerate=self.sample_rate,
                            blocksize=self.block_size,
                            dtype="float32",
                            latency="low",
                            callback=self._mon_out_callback,
                        )
                        self._mon_out_stream.start()
                        logger.info(
                            f"Started Monitor Output Stream on device {self.monitor_device}"
                        )
                    except Exception as e:
                        logger.warning(
                            f"Could not open monitor device {self.monitor_device}: {e}"
                        )

                # 3. Input Microphone Stream
                if self.input_device is not None:
                    self._in_stream = sd.InputStream(
                        device=self.input_device,
                        channels=1,
                        samplerate=self.sample_rate,
                        blocksize=self.block_size,
                        dtype="float32",
                        latency="low",
                        callback=self._input_callback,
                    )
                    self._in_stream.start()
                    logger.info(
                        f"Started Input Stream on device {self.input_device}"
                    )
                else:
                    logger.warning("No input microphone device found to open.")

                self.is_running = True
                return True
            except Exception as e:
                logger.error(f"Failed to start audio engine: {e}")
                self.stop()
                return False

    def stop(self):
        """Stops all active audio streams."""
        with self._lock:
            self.is_running = False

            if self._in_stream:
                try:
                    self._in_stream.stop()
                    self._in_stream.close()
                except Exception as e:
                    logger.debug(f"Error stopping in_stream: {e}")
                self._in_stream = None

            if self._virt_out_stream:
                try:
                    self._virt_out_stream.stop()
                    self._virt_out_stream.close()
                except Exception as e:
                    logger.debug(f"Error stopping virt_out_stream: {e}")
                self._virt_out_stream = None

            if self._mon_out_stream:
                try:
                    self._mon_out_stream.stop()
                    self._mon_out_stream.close()
                except Exception as e:
                    logger.debug(f"Error stopping mon_out_stream: {e}")
                self._mon_out_stream = None

            # Reset meters
            self.meter_input_peak = 0.0
            self.meter_input_rms = 0.0
            self.meter_output_peak = 0.0
            self.meter_output_rms = 0.0

            logger.info("AudioStreamEngine stopped.")

    def restart(self) -> bool:
        """Restarts the audio streams (e.g. after changing devices or buffer size)."""
        self.stop()
        time.sleep(0.05)
        return self.start()

    def _input_callback(self, indata, frames, time_info, status):
        """Processes incoming mic audio, applies DSP, mixes soundboard, and queues output."""
        if not self.is_running:
            return

        # 1. Raw Microphone input
        raw_mono = indata[:, 0]

        # Calculate input meter
        in_peak = float(np.max(np.abs(raw_mono)))
        in_rms = float(np.sqrt(np.mean(raw_mono**2) + 1e-12))
        self.meter_input_peak = in_peak
        self.meter_input_rms = in_rms

        # Apply mute / mic gain
        if self.is_muted:
            voice_block = np.zeros_like(raw_mono)
        else:
            voice_block = raw_mono * self.mic_gain

        # 2. Process Voice through DSP Pipeline
        processed_voice = self.dsp.process(voice_block)
        # Ensure (frames, 2) stereo float32
        if processed_voice.ndim == 1:
            processed_voice_stereo = np.column_stack(
                (processed_voice, processed_voice)
            )
        else:
            processed_voice_stereo = processed_voice

        # 3. Retrieve Soundboard active audio block
        soundboard_block = self.soundboard.get_mix_block(frames)
        if self.soundboard_gain != 1.0:
            soundboard_block = soundboard_block * self.soundboard_gain

        # 4. Final Mix for Virtual Microphone (Voice + Soundboard)
        virtual_mic_mix = processed_voice_stereo + soundboard_block
        # Soft limiter on final output
        virtual_mic_mix = np.tanh(virtual_mic_mix).astype(np.float32)

        # Calculate output meters
        out_peak = float(np.max(np.abs(virtual_mic_mix)))
        out_rms = float(np.sqrt(np.mean(virtual_mic_mix**2) + 1e-12))
        self.meter_output_peak = out_peak
        self.meter_output_rms = out_rms

        if self.on_meter_update:
            self.on_meter_update(in_peak, in_rms, out_peak, out_rms)

        # 5. Push to Virtual Sink Queue
        try:
            self._virt_queue.put_nowait(virtual_mic_mix)
        except queue.Full:
            try:
                self._virt_queue.get_nowait()
            except queue.Empty:
                pass
            try:
                self._virt_queue.put_nowait(virtual_mic_mix)
            except queue.Full:
                pass

        # 6. Prepare and Push Monitor (Headphone) audio
        if self._mon_out_stream is not None:
            mon_mix = np.zeros_like(virtual_mic_mix)
            if self.hear_myself:
                mon_mix += processed_voice_stereo
            if self.hear_soundboard:
                mon_mix += soundboard_block

            mon_mix = np.tanh(mon_mix * self.monitor_gain).astype(np.float32)
            try:
                self._mon_queue.put_nowait(mon_mix)
            except queue.Full:
                try:
                    self._mon_queue.get_nowait()
                except queue.Empty:
                    pass
                try:
                    self._mon_queue.put_nowait(mon_mix)
                except queue.Full:
                    pass

    def _virt_out_callback(self, outdata, frames, time_info, status):
        """Pulls mixed audio from queue and sends to Virtual Sink."""
        try:
            data = self._virt_queue.get_nowait()
            outdata[:] = data
        except queue.Empty:
            outdata.fill(0)

    def _mon_out_callback(self, outdata, frames, time_info, status):
        """Pulls monitor audio from queue and sends to Headphone/Speaker output."""
        try:
            data = self._mon_queue.get_nowait()
            outdata[:] = data
        except queue.Empty:
            outdata.fill(0)
