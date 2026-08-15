import logging
from PyQt6.QtCore import Qt, QTimer, pyqtSignal
from PyQt6.QtGui import QIcon
from PyQt6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QMainWindow,
    QProgressBar,
    QPushButton,
    QStackedWidget,
    QStatusBar,
    QVBoxLayout,
    QWidget,
)
from ..audio.dsp import VoiceDSP
from ..audio.router import AudioRouter
from ..audio.stream import AudioStreamEngine
from ..input.hotkeys import HotkeyManager
from ..soundboard.manager import SoundboardManager
from ..soundboard.player import SoundboardPlayer
from .audio_settings_panel import AudioSettingsPanel
from .hotkeys_panel import HotkeysPanel
from .soundboard_panel import SoundboardPanel
from .voice_panel import VoicePanel

logger = logging.getLogger("Audiover.MainWindow")


class MainWindow(QMainWindow):
    """Modern Dark & Neon Main Window for Audiover."""

    # Signal for thread-safe audio level updates to GUI
    meters_updated = pyqtSignal(float, float, float, float)

    def __init__(
        self,
        router: AudioRouter,
        dsp: VoiceDSP,
        stream_engine: AudioStreamEngine,
        soundboard_player: SoundboardPlayer,
        soundboard_manager: SoundboardManager,
        hotkey_manager: HotkeyManager,
    ):
        super().__init__()
        self.router = router
        self.dsp = dsp
        self.stream_engine = stream_engine
        self.player = soundboard_player
        self.sb_manager = soundboard_manager
        self.hotkey_mgr = hotkey_manager

        self.setWindowTitle("Audiover — Fedora Voice & Soundboard Engine")
        self.resize(1080, 720)
        self.setMinimumSize(900, 600)

        self.init_ui()
        self._connect_signals()

    def init_ui(self):
        central_widget = QWidget()
        self.setCentralWidget(central_widget)

        root_layout = QVBoxLayout(central_widget)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        # 1. Top Global Control & Metering Header
        header = QFrame()
        header.setStyleSheet(
            "background-color: #12141F; border-bottom: 1px solid #23263B; padding: 10px;"
        )
        h_layout = QHBoxLayout(header)
        h_layout.setContentsMargins(16, 8, 16, 8)
        h_layout.setSpacing(16)

        # Logo / Title
        title_box = QVBoxLayout()
        lbl_title = QLabel("AUDIOVER")
        lbl_title.setStyleSheet(
            "font-size: 18px; font-weight: 900; color: #00E5FF; letter-spacing: 2px;"
        )
        lbl_sub = QLabel("Fedora Voice & Soundboard Engine")
        lbl_sub.setStyleSheet("font-size: 11px; color: #8F9CAE;")
        title_box.addWidget(lbl_title)
        title_box.addWidget(lbl_sub)
        h_layout.addLayout(title_box)

        h_layout.addStretch()

        # Engine Power Toggle
        self.btn_power = QPushButton("● ENGINE ACTIVE")
        self.btn_power.setFixedHeight(40)
        self.btn_power.setCheckable(True)
        self.btn_power.setChecked(True)
        self.btn_power.setStyleSheet(
            "background-color: #00E676; color: #000; font-weight: bold; font-size: 13px;"
        )
        self.btn_power.clicked.connect(self._toggle_engine)
        h_layout.addWidget(self.btn_power)

        # Mute Mic Button
        self.btn_mute = QPushButton("🎙 Mute Mic")
        self.btn_mute.setFixedHeight(40)
        self.btn_mute.setCheckable(True)
        self.btn_mute.clicked.connect(self._toggle_mute)
        h_layout.addWidget(self.btn_mute)

        # Hear Myself (Loopback) Button
        self.btn_hear_myself = QPushButton("🎧 Hear Myself")
        self.btn_hear_myself.setFixedHeight(40)
        self.btn_hear_myself.setCheckable(True)
        self.btn_hear_myself.setChecked(self.stream_engine.hear_myself)
        self.btn_hear_myself.clicked.connect(self._toggle_hear_myself)
        h_layout.addWidget(self.btn_hear_myself)

        # Live VU Meters
        meter_box = QVBoxLayout()
        meter_box.setSpacing(4)

        in_meter_row = QHBoxLayout()
        in_meter_row.addWidget(
            QLabel("<small style='color: #8F9CAE;'>IN</small>")
        )
        self.meter_in = QProgressBar()
        self.meter_in.setRange(0, 100)
        self.meter_in.setValue(0)
        self.meter_in.setTextVisible(False)
        self.meter_in.setFixedSize(120, 10)
        in_meter_row.addWidget(self.meter_in)
        meter_box.addLayout(in_meter_row)

        out_meter_row = QHBoxLayout()
        out_meter_row.addWidget(
            QLabel("<small style='color: #8F9CAE;'>OUT</small>")
        )
        self.meter_out = QProgressBar()
        self.meter_out.setRange(0, 100)
        self.meter_out.setValue(0)
        self.meter_out.setTextVisible(False)
        self.meter_out.setFixedSize(120, 10)
        out_meter_row.addWidget(self.meter_out)
        meter_box.addLayout(out_meter_row)

        h_layout.addLayout(meter_box)

        root_layout.addWidget(header)

        # 2. Main Body (Sidebar + Content Stack)
        body = QWidget()
        body_layout = QHBoxLayout(body)
        body_layout.setContentsMargins(0, 0, 0, 0)
        body_layout.setSpacing(0)

        # Sidebar
        self.sidebar = QListWidget()
        self.sidebar.setObjectName("navSidebar")
        self.sidebar.setFixedWidth(210)
        self.sidebar.addItem("🎙  Voice Changer")
        self.sidebar.addItem("🔊  Soundboard")
        self.sidebar.addItem("⚙  Audio & Routing")
        self.sidebar.addItem("⌨  Global Hotkeys")
        self.sidebar.currentRowChanged.connect(self._on_tab_changed)
        body_layout.addWidget(self.sidebar)

        # Stacked Pages
        self.pages = QStackedWidget()

        self.voice_panel = VoicePanel(
            self.dsp, config_path=self.sb_manager.config_path
        )
        self.soundboard_panel = SoundboardPanel(self.sb_manager, self.player)
        self.audio_panel = AudioSettingsPanel(self.stream_engine, self.router)
        self.hotkeys_panel = HotkeysPanel(self.hotkey_mgr, self.sb_manager)

        self.pages.addWidget(self.voice_panel)
        self.pages.addWidget(self.soundboard_panel)
        self.pages.addWidget(self.audio_panel)
        self.pages.addWidget(self.hotkeys_panel)

        body_layout.addWidget(self.pages, stretch=1)
        root_layout.addWidget(body, stretch=1)

        # Select initial page
        self.sidebar.setCurrentRow(0)

        # Status Bar
        self.statusBar().showMessage(
            "Ready — PipeWire Virtual Microphone Active (Audiover_Mic)"
        )

    def _connect_signals(self):
        # Meter updates from audio stream engine
        self.meters_updated.connect(self._update_meters_gui)

        def _on_meter(in_peak, in_rms, out_peak, out_rms):
            self.meters_updated.emit(in_peak, in_rms, out_peak, out_rms)

        self.stream_engine.on_meter_update = _on_meter

        # Connect hotkeys
        self.hotkey_mgr.register_hotkey("F9", self._toggle_mute_from_hotkey)
        self.hotkey_mgr.register_hotkey("F10", self._toggle_bypass_from_hotkey)
        self.hotkey_mgr.register_hotkey("F11", self.player.stop_all)
        self.hotkey_mgr.register_hotkey(
            "F8", self._toggle_hear_myself_from_hotkey
        )

    def _update_meters_gui(
        self, in_peak: float, in_rms: float, out_peak: float, out_rms: float
    ):
        # Convert peak [0.0 - 1.0] to percentage with slight dynamic smoothing
        in_val = min(100, int(in_peak * 100))
        out_val = min(100, int(out_peak * 100))
        self.meter_in.setValue(in_val)
        self.meter_out.setValue(out_val)

    def _on_tab_changed(self, row):
        self.pages.setCurrentIndex(row)

    def _toggle_engine(self, checked):
        if checked:
            success = self.stream_engine.start()
            if success:
                self.btn_power.setText("● ENGINE ACTIVE")
                self.btn_power.setStyleSheet(
                    "background-color: #00E676; color: #000; font-weight: bold;"
                )
            else:
                self.btn_power.setChecked(False)
                self.btn_power.setText("○ ENGINE STOPPED")
                self.btn_power.setStyleSheet(
                    "background-color: #FF1744; color: #FFF; font-weight: bold;"
                )
        else:
            self.stream_engine.stop()
            self.btn_power.setText("○ ENGINE STOPPED")
            self.btn_power.setStyleSheet(
                "background-color: #FF1744; color: #FFF; font-weight: bold;"
            )

    def _toggle_mute(self, checked):
        self.stream_engine.is_muted = checked
        if checked:
            self.btn_mute.setText("🔇 Muted")
            self.btn_mute.setStyleSheet(
                "background-color: #FF1744; color: #FFF; font-weight: bold;"
            )
        else:
            self.btn_mute.setText("🎙 Mute Mic")
            self.btn_mute.setStyleSheet("")

    def _toggle_mute_from_hotkey(self):
        new_state = not self.stream_engine.is_muted
        self.btn_mute.setChecked(new_state)
        self._toggle_mute(new_state)

    def _toggle_bypass_from_hotkey(self):
        new_val = not self.voice_panel.chk_bypass.isChecked()
        self.voice_panel.chk_bypass.setChecked(new_val)

    def _toggle_hear_myself(self, checked):
        self.stream_engine.set_hear_myself(checked)
        self.audio_panel.chk_hear_myself.setChecked(checked)
        if checked:
            self.btn_hear_myself.setStyleSheet(
                "background-color: #7C4DFF; color: #FFF; font-weight: bold;"
            )
        else:
            self.btn_hear_myself.setStyleSheet("")

    def _toggle_hear_myself_from_hotkey(self):
        new_state = not self.stream_engine.hear_myself
        self.btn_hear_myself.setChecked(new_state)
        self._toggle_hear_myself(new_state)

    def closeEvent(self, event):
        logger.info("Shutting down Audiover application...")
        self.stream_engine.stop()
        self.hotkey_mgr.stop()
        self.player.stop_all()
        self.sb_manager.save_to_config()
        self.router.cleanup()
        event.accept()
