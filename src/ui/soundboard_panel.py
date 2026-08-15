import os
from PyQt6.QtCore import Qt, QTimer, pyqtSignal
from PyQt6.QtWidgets import (
    QCheckBox,
    QFileDialog,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QLineEdit,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QScrollArea,
    QSlider,
    QVBoxLayout,
    QWidget,
)
from ..soundboard.manager import SoundItem, SoundboardManager
from ..soundboard.player import SoundboardPlayer


class SoundCardWidget(QFrame):
    """Interactive Card representing a single Soundboard Track."""

    def __init__(
        self,
        item: SoundItem,
        player: SoundboardPlayer,
        manager: SoundboardManager,
        parent=None,
    ):
        super().__init__(parent)
        self.item = item
        self.player = player
        self.manager = manager
        self.setObjectName("soundCard")
        self.setProperty("class", "cardFrame")

        self.init_ui()

    def init_ui(self):
        self.setStyleSheet(
            """
            QFrame#soundCard {
                background-color: #181A27;
                border: 1px solid #282C40;
                border-radius: 10px;
                padding: 10px;
            }
            QFrame#soundCard:hover {
                border-color: #7C4DFF;
            }
        """
        )

        layout = QVBoxLayout(self)
        layout.setContentsMargins(10, 10, 10, 10)
        layout.setSpacing(8)

        # Header: Name & Hotkey
        header = QHBoxLayout()
        self.lbl_name = QLabel(self.item.name)
        self.lbl_name.setStyleSheet(
            "font-weight: bold; font-size: 14px; color: #FFFFFF;"
        )
        header.addWidget(self.lbl_name)

        header.addStretch()

        self.btn_hotkey = QPushButton(self.item.hotkey or "+ Key")
        self.btn_hotkey.setFixedWidth(70)
        self.btn_hotkey.setStyleSheet(
            "font-size: 11px; padding: 3px; background-color: #262B42; color: #00E5FF;"
        )
        self.btn_hotkey.clicked.connect(self._change_hotkey)
        header.addWidget(self.btn_hotkey)

        layout.addLayout(header)

        # Progress Bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 1000)
        self.progress_bar.setValue(0)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.setFixedHeight(6)
        layout.addWidget(self.progress_bar)

        # Controls Row
        ctrl_row = QHBoxLayout()
        ctrl_row.setSpacing(6)

        # Play / Pause
        self.btn_play = QPushButton("▶ Play")
        self.btn_play.setStyleSheet(
            "background-color: #00E676; color: #000; font-weight: bold;"
        )
        self.btn_play.clicked.connect(self._toggle_play)
        ctrl_row.addWidget(self.btn_play)

        # Stop
        self.btn_stop = QPushButton("■ Stop")
        self.btn_stop.clicked.connect(self._stop)
        ctrl_row.addWidget(self.btn_stop)

        # Loop checkbox
        self.chk_loop = QCheckBox("Loop")
        self.chk_loop.setChecked(self.item.loop)
        self.chk_loop.toggled.connect(self._toggle_loop)
        ctrl_row.addWidget(self.chk_loop)

        # Delete button
        self.btn_delete = QPushButton("✕")
        self.btn_delete.setFixedWidth(28)
        self.btn_delete.setStyleSheet(
            "color: #FF5252; background-color: transparent; border: none; font-size: 14px;"
        )
        self.btn_delete.clicked.connect(self._delete)
        ctrl_row.addWidget(self.btn_delete)

        layout.addLayout(ctrl_row)

        # Volume Slider
        vol_row = QHBoxLayout()
        vol_row.addWidget(QLabel("Vol:"))
        self.slider_vol = QSlider(Qt.Orientation.Horizontal)
        self.slider_vol.setRange(0, 150)
        self.slider_vol.setValue(int(self.item.volume * 100))
        self.slider_vol.valueChanged.connect(self._on_vol_changed)
        vol_row.addWidget(self.slider_vol)

        self.lbl_vol = QLabel(f"{int(self.item.volume*100)}%")
        self.lbl_vol.setFixedWidth(36)
        self.lbl_vol.setStyleSheet("color: #8F9CAE; font-size: 11px;")
        vol_row.addWidget(self.lbl_vol)

        layout.addLayout(vol_row)

    def update_progress(self):
        track = self.player.tracks.get(self.item.id)
        if track and track.is_playing:
            val = int(self.player.get_progress(self.item.id) * 1000)
            self.progress_bar.setValue(val)
            self.btn_play.setText("⏸ Pause")
            self.btn_play.setStyleSheet(
                "background-color: #FFD600; color: #000; font-weight: bold;"
            )
        else:
            self.progress_bar.setValue(0)
            self.btn_play.setText("▶ Play")
            self.btn_play.setStyleSheet(
                "background-color: #00E676; color: #000; font-weight: bold;"
            )

    def _toggle_play(self):
        track = self.player.tracks.get(self.item.id)
        if track and track.is_playing:
            self.player.pause(self.item.id)
        else:
            self.player.play(self.item.id, restart=True)

    def _stop(self):
        self.player.stop(self.item.id)
        self.progress_bar.setValue(0)

    def _toggle_loop(self, checked):
        self.manager.update_sound(self.item.id, loop=checked)

    def _on_vol_changed(self, val):
        vol = val / 100.0
        self.lbl_vol.setText(f"{val}%")
        self.manager.update_sound(self.item.id, volume=vol)

    def _change_hotkey(self):
        text, ok = QInputDialog.getText(
            self,
            "Assign Hotkey",
            f"Enter key or combination for '{self.item.name}' (e.g. F1, 1, NUM1):",
            text=self.item.hotkey or "",
        )
        if ok:
            clean = text.strip().upper()
            self.manager.update_sound(self.item.id, hotkey=clean)
            self.btn_hotkey.setText(clean or "+ Key")

    def _delete(self):
        reply = QMessageBox.question(
            self,
            "Remove Sound",
            f"Remove '{self.item.name}' from soundboard?",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if reply == QMessageBox.StandardButton.Yes:
            self.manager.remove_sound(self.item.id)
            self.setParent(None)
            self.deleteLater()


class SoundboardPanel(QWidget):
    """Grid/List View of Soundboard Clips with Search and Toolbar."""

    def __init__(
        self,
        manager: SoundboardManager,
        player: SoundboardPlayer,
        parent=None,
    ):
        super().__init__(parent)
        self.manager = manager
        self.player = player
        self.card_widgets = []

        self.init_ui()

        # Timer for UI progress bar updates
        self.progress_timer = QTimer(self)
        self.progress_timer.setInterval(40)  # 25 FPS
        self.progress_timer.timeout.connect(self._on_timer_tick)
        self.progress_timer.start()

    def init_ui(self):
        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(16, 16, 16, 16)
        main_layout.setSpacing(12)

        # 1. Top Controls Bar
        top_bar = QHBoxLayout()
        top_bar.setSpacing(10)

        # Add Sound Button
        self.btn_add = QPushButton("➕ Add Sound File (MP4/MP3/WAV)...")
        self.btn_add.setObjectName("primaryBtn")
        self.btn_add.setFixedHeight(38)
        self.btn_add.clicked.connect(self._on_add_sound_clicked)
        top_bar.addWidget(self.btn_add)

        # Stop All Panic Button
        self.btn_stop_all = QPushButton("⏹ STOP ALL SOUNDS")
        self.btn_stop_all.setObjectName("panicBtn")
        self.btn_stop_all.setFixedHeight(38)
        self.btn_stop_all.clicked.connect(self.player.stop_all)
        top_bar.addWidget(self.btn_stop_all)

        top_bar.addStretch()

        # Search Bar
        self.search_edit = QLineEdit()
        self.search_edit.setPlaceholderText("🔍 Search sounds...")
        self.search_edit.setFixedWidth(200)
        self.search_edit.textChanged.connect(self._filter_sounds)
        top_bar.addWidget(self.search_edit)

        main_layout.addLayout(top_bar)

        # 2. Scrollable Grid Area
        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setFrameShape(QFrame.Shape.NoFrame)

        self.grid_container = QWidget()
        self.grid_layout = QGridLayout(self.grid_container)
        self.grid_layout.setContentsMargins(0, 0, 0, 0)
        self.grid_layout.setSpacing(12)

        self.scroll.setWidget(self.grid_container)
        main_layout.addWidget(self.scroll)

        # Populate sounds
        self.refresh_sounds_grid()

    def refresh_sounds_grid(self):
        # Clear existing cards
        for card in self.card_widgets:
            card.setParent(None)
            card.deleteLater()
        self.card_widgets.clear()

        sounds = self.manager.get_all_sounds()
        query = self.search_edit.text().strip().lower()

        row, col = 0, 0
        max_cols = 3

        for item in sounds:
            if query and query not in item.name.lower():
                continue

            card = SoundCardWidget(item, self.player, self.manager)
            self.grid_layout.addWidget(card, row, col)
            self.card_widgets.append(card)

            col += 1
            if col >= max_cols:
                col = 0
                row += 1

        self.grid_layout.setRowStretch(row + 1, 1)

    def _filter_sounds(self, text):
        self.refresh_sounds_grid()

    def _on_add_sound_clicked(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "Select Sound or Video File",
            "",
            "Audio & Video Files (*.mp4 *.mp3 *.wav *.ogg *.flac *.m4a *.aac *.mkv *.mov);;All Files (*)",
        )
        if file_path:
            item = self.manager.add_sound_file(
                file_path=file_path,
                copy_to_assets=True,
            )
            if item:
                self.refresh_sounds_grid()
            else:
                QMessageBox.warning(
                    self,
                    "Decode Error",
                    f"Could not load and decode audio from: {os.path.basename(file_path)}",
                )

    def _on_timer_tick(self):
        for card in self.card_widgets:
            card.update_progress()
