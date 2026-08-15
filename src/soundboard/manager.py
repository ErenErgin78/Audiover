import json
import logging
import os
import shutil
import uuid
from dataclasses import asdict, dataclass
from typing import Dict, List, Optional
from .player import SoundboardPlayer, SoundTrack

logger = logging.getLogger("Audiover.SoundboardManager")


@dataclass
class SoundItem:
    id: str
    name: str
    file_path: str
    hotkey: Optional[str] = None
    volume: float = 1.0
    loop: bool = False
    category: str = "General"
    duration_sec: float = 0.0


class SoundboardManager:
    """Manages soundboard metadata, library organization, and settings persistence."""

    def __init__(
        self,
        config_path: str = "config/settings.json",
        sounds_dir: str = "assets/sounds",
        player: Optional[SoundboardPlayer] = None,
    ):
        self.config_path = config_path
        self.sounds_dir = sounds_dir
        self.player = player or SoundboardPlayer()
        self.sounds: Dict[str, SoundItem] = {}

        os.makedirs(self.sounds_dir, exist_ok=True)
        os.makedirs(os.path.dirname(self.config_path), exist_ok=True)

    def load_from_config(self):
        """Loads configured sounds from settings.json and pre-loads audio into player."""
        if not os.path.exists(self.config_path):
            return

        try:
            with open(self.config_path, "r", encoding="utf-8") as f:
                data = json.load(f)

            sound_list = data.get("soundboard", {}).get("sounds", [])
            for item in sound_list:
                s_id = item.get("id", str(uuid.uuid4()))
                file_path = item.get("file_path", "")
                if not os.path.exists(file_path):
                    # Check relative path inside assets/sounds
                    alt_path = os.path.join(
                        self.sounds_dir, os.path.basename(file_path)
                    )
                    if os.path.exists(alt_path):
                        file_path = alt_path
                    else:
                        logger.warning(
                            f"Sound file not found: {file_path}, skipping..."
                        )
                        continue

                sound_item = SoundItem(
                    id=s_id,
                    name=item.get(
                        "name",
                        os.path.splitext(os.path.basename(file_path))[0],
                    ),
                    file_path=file_path,
                    hotkey=item.get("hotkey"),
                    volume=float(item.get("volume", 1.0)),
                    loop=bool(item.get("loop", False)),
                    category=item.get("category", "General"),
                    duration_sec=float(item.get("duration_sec", 0.0)),
                )

                # Preload into player
                track = self.player.load_sound(
                    sound_id=s_id,
                    file_path=file_path,
                    name=sound_item.name,
                    volume=sound_item.volume,
                    loop=sound_item.loop,
                )
                if track:
                    sound_item.duration_sec = track.duration_sec
                    self.sounds[s_id] = sound_item

            logger.info(f"Loaded {len(self.sounds)} sounds into soundboard.")
        except Exception as e:
            logger.error(f"Error loading soundboard config: {e}")

    def save_to_config(self):
        """Persists soundboard metadata into settings.json."""
        try:
            settings = {}
            if os.path.exists(self.config_path):
                with open(self.config_path, "r", encoding="utf-8") as f:
                    settings = json.load(f)

            if "soundboard" not in settings:
                settings["soundboard"] = {}

            settings["soundboard"]["sounds"] = [
                asdict(item) for item in self.sounds.values()
            ]

            with open(self.config_path, "w", encoding="utf-8") as f:
                json.dump(settings, f, indent=2, ensure_ascii=False)
            logger.info("Saved soundboard configuration to settings.json.")
        except Exception as e:
            logger.error(f"Failed to save soundboard config: {e}")

    def add_sound_file(
        self,
        file_path: str,
        name: Optional[str] = None,
        copy_to_assets: bool = False,
        hotkey: Optional[str] = None,
        volume: float = 1.0,
        loop: bool = False,
        category: str = "General",
    ) -> Optional[SoundItem]:
        """Adds a sound to the library and loads it into memory."""
        if not os.path.exists(file_path):
            logger.error(f"File not found: {file_path}")
            return None

        sound_id = str(uuid.uuid4())[:8]
        if not name:
            name = os.path.splitext(os.path.basename(file_path))[0]

        target_path = file_path
        if copy_to_assets:
            dest_filename = f"{sound_id}_{os.path.basename(file_path)}"
            target_path = os.path.join(self.sounds_dir, dest_filename)
            shutil.copy2(file_path, target_path)

        track = self.player.load_sound(
            sound_id=sound_id,
            file_path=target_path,
            name=name,
            volume=volume,
            loop=loop,
        )

        if not track:
            return None

        sound_item = SoundItem(
            id=sound_id,
            name=name,
            file_path=target_path,
            hotkey=hotkey,
            volume=volume,
            loop=loop,
            category=category,
            duration_sec=track.duration_sec,
        )
        self.sounds[sound_id] = sound_item
        self.save_to_config()
        return sound_item

    def remove_sound(self, sound_id: str):
        """Removes a sound from library and player."""
        if sound_id in self.sounds:
            self.player.remove_sound(sound_id)
            del self.sounds[sound_id]
            self.save_to_config()

    def update_sound(
        self,
        sound_id: str,
        name: Optional[str] = None,
        hotkey: Optional[str] = None,
        volume: Optional[float] = None,
        loop: Optional[bool] = None,
    ):
        """Updates metadata and syncs with player."""
        item = self.sounds.get(sound_id)
        if not item:
            return

        if name is not None:
            item.name = name
        if hotkey is not None:
            item.hotkey = hotkey if hotkey.strip() else None
        if volume is not None:
            item.volume = volume
            self.player.set_volume(sound_id, volume)
        if loop is not None:
            item.loop = loop
            self.player.set_loop(sound_id, loop)

        self.save_to_config()

    def get_sound(self, sound_id: str) -> Optional[SoundItem]:
        return self.sounds.get(sound_id)

    def get_all_sounds(self) -> List[SoundItem]:
        return list(self.sounds.values())
