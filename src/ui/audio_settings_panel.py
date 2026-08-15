from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QFrame,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QSlider,
    QVBoxLayout,
    QWidget,
)
from ..audio.router import AudioRouter
from ..audio.stream import AudioStreamEngine


class AudioSettingsPanel(QWidget):
    """Configuration Panel for Audio Devices, PipeWire Routing & Latency."""

    settings_changed = pyqtSignal()

    def __init__(
        self,
        stream_engine: AudioStreamEngine,
        router: AudioRouter,
        parent=None,
    ):
        super().__init__(parent)
        self.stream_engine = stream_engine
        self.router = router
        self.input_device_indices = []
        self.output_device_indices = []

        self.init_ui()
        self.refresh_devices()

    def init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 16, 16, 16)
        layout.setSpacing(16)

        # 1. Device Selection Group
        dev_group = QGroupBox("Audio Devices & Hardware I/O")
        dg_layout = QVBoxLayout(dev_group)
        dg_layout.setSpacing(12)

        # Input Mic
        in_row = QHBoxLayout()
        in_row.addWidget(QLabel("Physical Microphone Input:"))
        self.combo_input = QComboBox()
        self.combo_input.currentIndexChanged.connect(self._on_input_changed)
        in_row.addWidget(self.combo_input, stretch=1)
        dg_layout.addLayout(in_row)

        # Monitor Headphones
        out_row = QHBoxLayout()
        out_row.addWidget(QLabel("Headphones / Monitor Output:"))
        self.combo_monitor = QComboBox()
        self.combo_monitor.currentIndexChanged.connect(self._on_monitor_changed)
        out_row.addWidget(self.combo_monitor, stretch=1)
        dg_layout.addLayout(out_row)

        # Refresh Devices button
        btn_refresh = QPushButton("🔄 Refresh Audio Devices")
        btn_refresh.clicked.connect(self.refresh_devices)
        dg_layout.addWidget(btn_refresh)

        layout.addWidget(dev_group)

        # 2. Virtual Microphone & PipeWire Status Card
        pw_group = QGroupBox("PipeWire Virtual Audio Routing")
        pw_layout = QVBoxLayout(pw_group)
        pw_layout.setSpacing(8)

        status_text = (
            "<b>PipeWire Virtual Sink:</b> <code>Audiover_Sink</code><br>"
            "<b>PipeWire Virtual Microphone:</b> <code>Audiover_Mic</code><br>"
            "<small style='color: #8F9CAE;'>Select <b>Audiover_Virtual_Microphone</b> in Discord, OBS, Games, or Telegram.</small>"
        )
        self.lbl_pw_status = QLabel(status_text)
        self.lbl_pw_status.setTextFormat(Qt.TextFormat.RichText)
        pw_layout.addWidget(self.lbl_pw_status)

        layout.addWidget(pw_group)

        # 3. Latency & Buffer Size Group
        buf_group = QGroupBox("Latency & Performance")
        bg_layout = QVBoxLayout(buf_group)

        buf_row = QHBoxLayout()
        buf_row.addWidget(QLabel("Buffer Size (Frames):"))
        self.combo_buf = QComboBox()
        self.combo_buf.addItems(
            [
                "128 samples (~2.7 ms)",
                "256 samples (~5.3 ms) [Recommended]",
                "512 samples (~10.7 ms)",
                "1024 samples (~21.3 ms)",
            ]
        )
        self.combo_buf.setCurrentIndex(1)  # 256
        self.combo_buf.currentIndexChanged.connect(self._on_buffer_size_changed)
        buf_row.addWidget(self.combo_buf, stretch=1)
        bg_layout.addLayout(buf_row)

        layout.addWidget(buf_group)

        # 4. Monitoring & Gain Controls Group
        gain_group = QGroupBox("Audio Levels & Monitoring")
        gg_layout = QVBoxLayout(gain_group)
        gg_layout.setSpacing(12)

        # Toggles
        tog_row = QHBoxLayout()
        self.chk_hear_myself = QCheckBox(
            "Hear Myself (Microphone Loopback to Headphones)"
        )
        self.chk_hear_myself.setChecked(self.stream_engine.hear_myself)
        self.chk_hear_myself.toggled.connect(self._on_hear_myself_toggled)
        tog_row.addWidget(self.chk_hear_myself)

        self.chk_hear_sb = QCheckBox(
            "Hear Soundboard (Soundboard to Headphones)"
        )
        self.chk_hear_sb.setChecked(self.stream_engine.hear_soundboard)
        self.chk_hear_sb.toggled.connect(self._on_hear_sb_toggled)
        tog_row.addWidget(self.chk_hear_sb)
        gg_layout.addLayout(tog_row)

        # Mic Gain
        mg_row = QHBoxLayout()
        mg_row.addWidget(QLabel("Mic Input Volume:"))
        self.slider_mic_gain = QSlider(Qt.Orientation.Horizontal)
        self.slider_mic_gain.setRange(0, 200)
        self.slider_mic_gain.setValue(100)
        self.slider_mic_gain.valueChanged.connect(self._on_mic_gain_changed)
        mg_row.addWidget(self.slider_mic_gain)
        self.lbl_mic_gain = QLabel("100%")
        self.lbl_mic_gain.setFixedWidth(45)
        mg_row.addWidget(self.lbl_mic_gain)
        gg_layout.addLayout(mg_row)

        # Monitor Gain
        mong_row = QHBoxLayout()
        mong_row.addWidget(QLabel("Headphone Monitor Volume:"))
        self.slider_mon_gain = QSlider(Qt.Orientation.Horizontal)
        self.slider_mon_gain.setRange(0, 200)
        self.slider_mon_gain.setValue(100)
        self.slider_mon_gain.valueChanged.connect(self._on_mon_gain_changed)
        mong_row.addWidget(self.slider_mon_gain)
        self.lbl_mon_gain = QLabel("100%")
        self.lbl_mon_gain.setFixedWidth(45)
        mong_row.addWidget(self.lbl_mon_gain)
        gg_layout.addLayout(mong_row)

        layout.addWidget(gain_group)

        layout.addStretch()

    def refresh_devices(self):
        inputs, outputs = self.router.get_audio_devices()

        self.combo_input.blockSignals(True)
        self.combo_monitor.blockSignals(True)

        self.combo_input.clear()
        self.input_device_indices.clear()

        selected_in_idx = 0
        current_in = self.stream_engine.input_device

        for dev in inputs:
            # Skip Audiover virtual devices and application JACK endpoints
            name_l = dev["name"].lower()
            if any(
                k in name_l
                for k in ["audiover", "brave", "chromium", "zapzap", "firefox"]
            ):
                continue
            self.combo_input.addItem(
                f"[{dev['index']}] {dev['name']}"
                + (" (Default)" if dev["is_default"] else "")
            )
            self.input_device_indices.append(dev["index"])
            if current_in is not None and dev["index"] == current_in:
                selected_in_idx = len(self.input_device_indices) - 1
            elif current_in is None and dev["is_default"]:
                selected_in_idx = len(self.input_device_indices) - 1

        if self.input_device_indices:
            self.combo_input.setCurrentIndex(selected_in_idx)
            chosen_dev = self.input_device_indices[selected_in_idx]
            if self.stream_engine.input_device != chosen_dev:
                self.stream_engine.input_device = chosen_dev
                if self.stream_engine.is_running:
                    self.stream_engine.restart()

        self.combo_monitor.clear()
        self.output_device_indices.clear()
        # Add None / Disabled option
        self.combo_monitor.addItem("Disabled / None")
        self.output_device_indices.append(None)

        selected_out_idx = 0
        current_mon = self.stream_engine.monitor_device

        for dev in outputs:
            # Skip Audiover virtual sink in monitor dropdown
            if "Audiover" in dev["name"]:
                continue
            self.combo_monitor.addItem(
                f"[{dev['index']}] {dev['name']}"
                + (" (Default)" if dev["is_default"] else "")
            )
            self.output_device_indices.append(dev["index"])
            if current_mon is not None and dev["index"] == current_mon:
                selected_out_idx = len(self.output_device_indices) - 1
            elif current_mon is None and dev["is_default"]:
                selected_out_idx = len(self.output_device_indices) - 1

        if self.output_device_indices:
            if selected_out_idx == 0 and len(self.output_device_indices) > 1:
                # If Disabled was selected by default but audio output devices exist, choose the first physical device
                selected_out_idx = 1
            self.combo_monitor.setCurrentIndex(selected_out_idx)
            chosen_mon = self.output_device_indices[selected_out_idx]
            if (
                chosen_mon is not None
                and self.stream_engine.monitor_device != chosen_mon
            ):
                self.stream_engine.monitor_device = chosen_mon
                if self.stream_engine.is_running:
                    self.stream_engine.restart()

        self.combo_input.blockSignals(False)
        self.combo_monitor.blockSignals(False)

    def _on_input_changed(self, index):
        if 0 <= index < len(self.input_device_indices):
            dev_idx = self.input_device_indices[index]
            self.stream_engine.input_device = dev_idx
            if self.stream_engine.is_running:
                self.stream_engine.restart()

    def _on_monitor_changed(self, index):
        if 0 <= index < len(self.output_device_indices):
            dev_idx = self.output_device_indices[index]
            self.stream_engine.monitor_device = dev_idx
            if self.stream_engine.is_running:
                self.stream_engine.restart()

    def _on_buffer_size_changed(self, index):
        sizes = [128, 256, 512, 1024]
        if 0 <= index < len(sizes):
            self.stream_engine.block_size = sizes[index]
            if self.stream_engine.is_running:
                self.stream_engine.restart()

    def _on_hear_myself_toggled(self, checked):
        self.stream_engine.set_hear_myself(checked)

    def _on_hear_sb_toggled(self, checked):
        self.stream_engine.hear_soundboard = checked

    def _on_mic_gain_changed(self, val):
        gain = val / 100.0
        self.lbl_mic_gain.setText(f"{val}%")
        self.stream_engine.mic_gain = gain

    def _on_mon_gain_changed(self, val):
        gain = val / 100.0
        self.lbl_mon_gain.setText(f"{val}%")
        self.stream_engine.monitor_gain = gain
