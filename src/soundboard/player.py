import logging
import os
import subprocess
import threading
from dataclasses import dataclass
from typing import Callable, Dict, List, Optional
import numpy as np
import scipy.signal
import soundfile as sf

logger = logging.getLogger("Audiover.SoundboardPlayer")


@dataclass
class SoundTrack:
    sound_id: str
    name: str
    file_path: str
    audio_data: np.ndarray  # float32 array of shape (N, 2)
    sample_rate: int
    duration_sec: float
    position: int = 0
    volume: float = 1.0
    loop: bool = False
    is_playing: bool = False


class SoundboardPlayer:
    """High-performance in-memory multi-track soundboard audio player and mixer."""

    def __init__(self, target_sample_rate: int = 48000):
        self.target_sr = target_sample_rate
        self.tracks: Dict[str, SoundTrack] = {}
        self.lock = threading.Lock()
        self.on_track_finished: Optional[Callable[[str], None]] = None
        self.on_playback_state_changed: Optional[
            Callable[[str, bool], None]
        ] = None

    def load_sound(
        self,
        sound_id: str,
        file_path: str,
        name: Optional[str] = None,
        volume: float = 1.0,
        loop: bool = False,
    ) -> Optional[SoundTrack]:
        """Loads and decodes an audio or video file into 48kHz float32 stereo array in RAM."""
        if not os.path.exists(file_path):
            logger.error(f"Sound file not found: {file_path}")
            return None

        sound_name = (
            name
            if name
            else os.path.splitext(os.path.basename(file_path))[0]
        )

        try:
            audio_data, sr = self._decode_file(file_path)
            if audio_data is None:
                return None

            # Resample to target sample rate if needed
            if sr != self.target_sr:
                num_target_samples = int(
                    len(audio_data) * self.target_sr / sr
                )
                audio_data = scipy.signal.resample(
                    audio_data, num_target_samples, axis=0
                )
                sr = self.target_sr

            # Ensure shape is (N, 2) stereo float32
            if audio_data.ndim == 1:
                audio_data = np.column_stack((audio_data, audio_data))
            elif audio_data.ndim == 2 and audio_data.shape[1] == 1:
                audio_data = np.column_stack((audio_data[:, 0], audio_data[:, 0]))
            elif audio_data.ndim == 2 and audio_data.shape[1] > 2:
                audio_data = audio_data[:, :2]

            audio_data = audio_data.astype(np.float32)
            duration_sec = len(audio_data) / self.target_sr

            track = SoundTrack(
                sound_id=sound_id,
                name=sound_name,
                file_path=file_path,
                audio_data=audio_data,
                sample_rate=self.target_sr,
                duration_sec=duration_sec,
                volume=volume,
                loop=loop,
                is_playing=False,
            )

            with self.lock:
                self.tracks[sound_id] = track

            logger.info(
                f"Loaded sound '{sound_name}' ({duration_sec:.2f}s, {len(audio_data)} samples)"
            )
            return track
        except Exception as e:
            logger.error(f"Failed to load sound '{file_path}': {e}")
            return None

    def _decode_file(self, file_path: str) -> (Optional[np.ndarray], int):
        """Decodes audio/video using soundfile, miniaudio, or ffmpeg."""
        ext = os.path.splitext(file_path)[1].lower()

        # Direct soundfile decode for WAV / FLAC / OGG
        if ext in [".wav", ".flac", ".ogg"]:
            try:
                data, sr = sf.read(file_path, dtype="float32")
                return data, sr
            except Exception as e:
                logger.debug(f"soundfile failed on {file_path}: {e}")

        # Try miniaudio for MP3 / WAV / FLAC
        try:
            import miniaudio

            decoded = miniaudio.decode_file(
                file_path,
                nchannels=2,
                sample_rate=self.target_sr,
                output_format=miniaudio.SampleFormat.FLOAT32,
            )
            data = np.frombuffer(decoded.samples, dtype=np.float32).reshape(
                -1, 2
            )
            return data, self.target_sr
        except Exception as e:
            logger.debug(f"miniaudio failed on {file_path}: {e}")

        # FFmpeg fallback for MP4, M4A, MOV, MKV, etc.
        try:
            cmd = [
                "ffmpeg",
                "-v",
                "error",
                "-i",
                file_path,
                "-f",
                "f32le",
                "-ac",
                "2",
                "-ar",
                str(self.target_sr),
                "-",
            ]
            res = subprocess.run(
                cmd, capture_output=True, check=True, timeout=10
            )
            data = np.frombuffer(res.stdout, dtype=np.float32).reshape(-1, 2)
            return data, self.target_sr
        except Exception as e:
            logger.error(f"FFmpeg decode failed on {file_path}: {e}")

        return None, 0

    def play(self, sound_id: str, restart: bool = True):
        """Starts or restarts playback of a sound."""
        with self.lock:
            track = self.tracks.get(sound_id)
            if not track:
                logger.warning(f"Sound ID not found: {sound_id}")
                return
            if restart:
                track.position = 0
            track.is_playing = True

        if self.on_playback_state_changed:
            self.on_playback_state_changed(sound_id, True)

    def pause(self, sound_id: str):
        with self.lock:
            track = self.tracks.get(sound_id)
            if track:
                track.is_playing = False

        if self.on_playback_state_changed:
            self.on_playback_state_changed(sound_id, False)

    def stop(self, sound_id: str):
        with self.lock:
            track = self.tracks.get(sound_id)
            if track:
                track.is_playing = False
                track.position = 0

        if self.on_playback_state_changed:
            self.on_playback_state_changed(sound_id, False)

    def stop_all(self):
        """Immediately stops all playing tracks."""
        stopped_ids = []
        with self.lock:
            for sound_id, track in self.tracks.items():
                if track.is_playing:
                    track.is_playing = False
                    track.position = 0
                    stopped_ids.append(sound_id)

        if self.on_playback_state_changed:
            for sid in stopped_ids:
                self.on_playback_state_changed(sid, False)

    def set_volume(self, sound_id: str, volume: float):
        with self.lock:
            track = self.tracks.get(sound_id)
            if track:
                track.volume = max(0.0, min(volume, 2.0))

    def set_loop(self, sound_id: str, loop: bool):
        with self.lock:
            track = self.tracks.get(sound_id)
            if track:
                track.loop = loop

    def remove_sound(self, sound_id: str):
        with self.lock:
            if sound_id in self.tracks:
                del self.tracks[sound_id]

    def get_progress(self, sound_id: str) -> float:
        """Returns normalized progress [0.0 - 1.0] of a sound."""
        with self.lock:
            track = self.tracks.get(sound_id)
            if not track or len(track.audio_data) == 0:
                return 0.0
            return track.position / len(track.audio_data)

    def get_mix_block(self, block_size: int) -> np.ndarray:
        """Mixes all active playing tracks for the current audio block.

        Returns:
            np.ndarray of shape (block_size, 2) in float32.
        """
        mix = np.zeros((block_size, 2), dtype=np.float32)
        finished_tracks = []

        with self.lock:
            for sound_id, track in self.tracks.items():
                if not track.is_playing:
                    continue

                total_samples = len(track.audio_data)
                curr_pos = track.position
                samples_needed = block_size
                block_offset = 0

                while samples_needed > 0 and track.is_playing:
                    available = total_samples - curr_pos
                    if available <= 0:
                        if track.loop:
                            curr_pos = 0
                            available = total_samples
                        else:
                            track.is_playing = False
                            track.position = 0
                            finished_tracks.append(sound_id)
                            break

                    chunk_len = min(samples_needed, available)
                    mix[block_offset : block_offset + chunk_len] += (
                        track.audio_data[curr_pos : curr_pos + chunk_len]
                        * track.volume
                    )

                    curr_pos += chunk_len
                    block_offset += chunk_len
                    samples_needed -= chunk_len

                track.position = curr_pos

        if finished_tracks and self.on_track_finished:
            for sid in finished_tracks:
                self.on_track_finished(sid)

        return mix
