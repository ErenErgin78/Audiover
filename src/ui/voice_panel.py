import json
import logging
import os
from typing import Dict, Optional
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QFrame,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QMessageBox,
    QPushButton,
    QScrollArea,
    QSlider,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)
from ..audio.dsp import DSPOptions, VoiceDSP

logger = logging.getLogger("Audiover.VoicePanel")

DEFAULT_PRESETS: Dict[str, dict] = {
    "Clean": {
        "pitch": 0.0,
        "robot": False,
        "rfreq": 150,
        "rmix": 0.0,
        "radio": False,
        "dist": False,
        "drive": 0.0,
        "rev": False,
        "rsize": 0.0,
        "rwet": 0.0,
        "chorus": False,
        "cdepth": 0.0,
    },
    "Deep Voice": {
        "pitch": -5.5,
        "robot": False,
        "rfreq": 120,
        "rmix": 0.0,
        "radio": False,
        "dist": True,
        "drive": 0.20,
        "rev": True,
        "rsize": 0.40,
        "rwet": 0.25,
        "chorus": False,
        "cdepth": 0.0,
    },
}


class VoicePanel(QWidget):
    """Interactive Voice Changer with prominent main preset cards and a dedicated DSP settings view."""

    preset_changed = pyqtSignal(str)

    def __init__(
        self,
        dsp: VoiceDSP,
        config_path: str = "config/settings.json",
        parent=None,
    ):
        super().__init__(parent)
        self.dsp = dsp
        self.config_path = config_path
        self.custom_presets: Dict[str, dict] = {}
        self.preset_buttons: Dict[str, QPushButton] = {}
        self.active_preset: str = "Clean"
        self._is_loading_preset: bool = False

        self.load_custom_presets()
        self.init_ui()

    @property
    def presets(self) -> Dict[str, dict]:
        all_presets = dict(DEFAULT_PRESETS)
        all_presets.update(self.custom_presets)
        return all_presets

    def load_custom_presets(self):
        """Loads custom presets from configuration file."""
        if not os.path.exists(self.config_path):
            return

        try:
            with open(self.config_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            self.custom_presets = data.get("voice_effects", {}).get(
                "custom_presets", {}
            )
        except Exception as e:
            logger.error(f"Error loading custom voice presets: {e}")

    def save_custom_presets(self):
        """Persists custom presets into configuration file."""
        try:
            settings = {}
            if os.path.exists(self.config_path):
                with open(self.config_path, "r", encoding="utf-8") as f:
                    settings = json.load(f)

            if "voice_effects" not in settings:
                settings["voice_effects"] = {}

            settings["voice_effects"]["custom_presets"] = self.custom_presets

            os.makedirs(os.path.dirname(self.config_path), exist_ok=True)
            with open(self.config_path, "w", encoding="utf-8") as f:
                json.dump(settings, f, indent=2, ensure_ascii=False)
            logger.info("Saved custom voice presets to settings.json.")
        except Exception as e:
            logger.error(f"Failed to save custom voice presets: {e}")

    def init_ui(self):
        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self.stack = QStackedWidget()
        root_layout.addWidget(self.stack)

        # Page 0: Main Presets View (Prominent & Centered)
        self.page_presets = self._create_main_presets_view()
        self.stack.addWidget(self.page_presets)

        # Page 1: DSP Master Controls & Preset Editor View
        self.page_settings = self._create_settings_view()
        self.stack.addWidget(self.page_settings)

        # Initial render & preset
        self._rebuild_main_preset_cards()
        self.apply_preset("Clean")

    # ==========================================
    # 1. MAIN PRESETS VIEW
    # ==========================================
    def _create_main_presets_view(self) -> QWidget:
        view = QWidget()
        layout = QVBoxLayout(view)
        layout.setContentsMargins(24, 20, 24, 24)
        layout.setSpacing(20)

        # Top Bar: Title & Settings Button
        top_bar = QHBoxLayout()
        top_bar.setSpacing(12)

        header_box = QVBoxLayout()
        header_box.setSpacing(2)
        lbl_title = QLabel("Voice Presets")
        lbl_title.setStyleSheet(
            "font-size: 20px; font-weight: 900; color: #00E5FF; letter-spacing: 1px;"
        )
        self.lbl_subtitle = QLabel("Select an instant voice transformation preset")
        self.lbl_subtitle.setStyleSheet("font-size: 12px; color: #8F9CAE;")
        header_box.addWidget(lbl_title)
        header_box.addWidget(self.lbl_subtitle)
        top_bar.addLayout(header_box)

        top_bar.addStretch()

        btn_settings = QPushButton("⚙  Preset & DSP Settings")
        btn_settings.setFixedHeight(42)
        btn_settings.setStyleSheet(
            "QPushButton { background-color: #1E2235; border: 1px solid #363B59; "
            "border-radius: 10px; padding: 0 18px; color: #00E5FF; font-weight: bold; font-size: 13px; } "
            "QPushButton:hover { background-color: #2D324D; border-color: #00E5FF; color: #FFFFFF; }"
        )
        btn_settings.clicked.connect(self._open_settings_page)
        top_bar.addWidget(btn_settings)

        layout.addLayout(top_bar)

        # Centered Cards Container
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.setStyleSheet("background: transparent;")

        center_wrapper = QWidget()
        center_wrapper_layout = QVBoxLayout(center_wrapper)
        center_wrapper_layout.setContentsMargins(10, 20, 10, 20)
        center_wrapper_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

        self.cards_container = QWidget()
        self.cards_layout = QGridLayout(self.cards_container)
        self.cards_layout.setSpacing(24)
        self.cards_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

        center_wrapper_layout.addWidget(
            self.cards_container, alignment=Qt.AlignmentFlag.AlignCenter
        )
        scroll.setWidget(center_wrapper)
        layout.addWidget(scroll, stretch=1)

        return view

    def _rebuild_main_preset_cards(self):
        """Rebuilds the large, prominent preset cards in the main view."""
        self._clear_layout(self.cards_layout)
        self.preset_buttons.clear()

        all_presets = self.presets
        preset_names = list(all_presets.keys())

        # Determine grid column layout (2 or 3 columns)
        cols = 2 if len(preset_names) <= 4 else 3
        row, col = 0, 0

        for name in preset_names:
            is_custom = name not in DEFAULT_PRESETS
            icon = "★" if is_custom else ("🎙" if name == "Clean" else "🔊")
            subtext = "Custom Preset" if is_custom else ("Natural Microphone" if name == "Clean" else "Deep Studio Bass")

            btn = QPushButton()
            btn.setCheckable(True)
            btn.setFixedSize(220, 130)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)

            # Card Content (HTML rich text formatted label inside button)
            card_layout = QVBoxLayout(btn)
            card_layout.setContentsMargins(12, 14, 12, 14)
            card_layout.setSpacing(4)
            card_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

            lbl_icon = QLabel(icon)
            lbl_icon.setStyleSheet(
                "font-size: 26px; background: transparent; border: none;"
            )
            lbl_icon.setAlignment(Qt.AlignmentFlag.AlignCenter)

            lbl_name = QLabel(name)
            lbl_name.setStyleSheet(
                "font-size: 16px; font-weight: 900; color: #FFFFFF; background: transparent; border: none;"
            )
            lbl_name.setAlignment(Qt.AlignmentFlag.AlignCenter)

            lbl_desc = QLabel(subtext)
            lbl_desc.setStyleSheet(
                "font-size: 11px; color: #8F9CAE; background: transparent; border: none;"
            )
            lbl_desc.setAlignment(Qt.AlignmentFlag.AlignCenter)

            card_layout.addWidget(lbl_icon)
            card_layout.addWidget(lbl_name)
            card_layout.addWidget(lbl_desc)

            btn.clicked.connect(lambda checked, n=name: self.apply_preset(n))

            self.cards_layout.addWidget(btn, row, col)
            self.preset_buttons[name] = btn

            col += 1
            if col >= cols:
                col = 0
                row += 1

        self._highlight_active_preset(self.active_preset)

    def _highlight_active_preset(self, preset_name: str):
        for name, btn in self.preset_buttons.items():
            if name == preset_name:
                btn.setChecked(True)
                btn.setStyleSheet(
                    "QPushButton { "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #7C4DFF, stop:1 #00E5FF); "
                    "border: 2px solid #00E5FF; border-radius: 16px; "
                    "} "
                    "QPushButton:hover { "
                    "background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 #8E65FF, stop:1 #33ECFF); "
                    "border: 2px solid #FFFFFF; "
                    "}"
                )
            else:
                btn.setChecked(False)
                btn.setStyleSheet(
                    "QPushButton { "
                    "background-color: #181A27; border: 2px solid #282C40; border-radius: 16px; "
                    "} "
                    "QPushButton:hover { "
                    "background-color: #23273C; border: 2px solid #00E5FF; "
                    "}"
                )

        self.lbl_subtitle.setText(f"Active preset: {preset_name}")

    # ==========================================
    # 2. SETTINGS & DSP MASTER CONTROLS VIEW
    # ==========================================
    def _create_settings_view(self) -> QWidget:
        view = QWidget()
        layout = QVBoxLayout(view)
        layout.setContentsMargins(20, 16, 20, 16)
        layout.setSpacing(14)

        # Top Bar: Back Button, Title, Preset Selector & Actions
        top_bar = QHBoxLayout()
        top_bar.setSpacing(12)

        btn_back = QPushButton("← Back to Presets")
        btn_back.setFixedHeight(38)
        btn_back.setStyleSheet(
            "QPushButton { background-color: #23273C; border: 1px solid #363B59; border-radius: 8px; "
            "padding: 0 14px; font-weight: bold; color: #E2E8F0; } "
            "QPushButton:hover { background-color: #2D324D; border-color: #00E5FF; color: #FFFFFF; }"
        )
        btn_back.clicked.connect(self._close_settings_page)
        top_bar.addWidget(btn_back)

        top_bar.addSpacing(8)

        lbl_settings_title = QLabel("Preset & DSP Master Controls")
        lbl_settings_title.setStyleSheet(
            "font-size: 16px; font-weight: bold; color: #00E5FF;"
        )
        top_bar.addWidget(lbl_settings_title)

        top_bar.addStretch()

        # Preset Management Toolbar
        top_bar.addWidget(QLabel("Editing Preset:"))
        self.combo_edit_preset = QComboBox()
        self.combo_edit_preset.setMinimumWidth(150)
        self.combo_edit_preset.setFixedHeight(38)
        self.combo_edit_preset.currentTextChanged.connect(
            self._on_preset_combo_selected
        )
        top_bar.addWidget(self.combo_edit_preset)

        btn_add = QPushButton("➕ New Preset")
        btn_add.setFixedHeight(38)
        btn_add.setStyleSheet(
            "QPushButton { background-color: #1A2738; color: #00E5FF; border: 1px solid #00E5FF; "
            "font-weight: bold; border-radius: 8px; padding: 0 12px; } "
            "QPushButton:hover { background-color: #00E5FF; color: #0F111A; }"
        )
        btn_add.clicked.connect(self.create_new_preset)
        top_bar.addWidget(btn_add)

        self.btn_save_preset = QPushButton("💾 Save to Preset")
        self.btn_save_preset.setFixedHeight(38)
        self.btn_save_preset.setStyleSheet(
            "QPushButton { background-color: #7C4DFF; color: #FFFFFF; font-weight: bold; "
            "border-radius: 8px; padding: 0 12px; } "
            "QPushButton:hover { background-color: #8E65FF; }"
        )
        self.btn_save_preset.clicked.connect(self.save_current_to_selected_preset)
        top_bar.addWidget(self.btn_save_preset)

        self.btn_delete_preset = QPushButton("🗑 Delete")
        self.btn_delete_preset.setFixedHeight(38)
        self.btn_delete_preset.setStyleSheet(
            "QPushButton { background-color: #2D1A24; color: #FF5252; border: 1px solid #5A2030; "
            "font-weight: bold; border-radius: 8px; padding: 0 12px; } "
            "QPushButton:hover { background-color: #FF1744; color: #FFFFFF; border-color: #FF1744; } "
            "QPushButton:disabled { background-color: #1A1C27; color: #4A5168; border-color: #23263B; }"
        )
        self.btn_delete_preset.clicked.connect(self.delete_selected_preset)
        top_bar.addWidget(self.btn_delete_preset)

        layout.addLayout(top_bar)

        # Scrollable Detailed DSP Controls
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)

        controls_container = QWidget()
        ctrl_layout = QVBoxLayout(controls_container)
        ctrl_layout.setContentsMargins(0, 8, 0, 8)
        ctrl_layout.setSpacing(14)

        # 1. Master & Pitch Group
        pitch_group = QGroupBox("Pitch Shifter & Master Controls")
        pg_layout = QVBoxLayout(pitch_group)

        self.chk_bypass = QCheckBox("Bypass All DSP Effects (Original Clean Voice)")
        self.chk_bypass.toggled.connect(self._on_controls_changed)
        pg_layout.addWidget(self.chk_bypass)

        p_row = QHBoxLayout()
        p_row.addWidget(QLabel("Pitch Shift:"))
        self.slider_pitch = QSlider(Qt.Orientation.Horizontal)
        self.slider_pitch.setRange(-120, 120)
        self.slider_pitch.setValue(0)
        self.slider_pitch.valueChanged.connect(self._on_pitch_slider_changed)
        p_row.addWidget(self.slider_pitch)

        self.lbl_pitch_val = QLabel("0.0 st")
        self.lbl_pitch_val.setFixedWidth(55)
        p_row.addWidget(self.lbl_pitch_val)

        btn_reset_pitch = QPushButton("Reset")
        btn_reset_pitch.setFixedWidth(60)
        btn_reset_pitch.clicked.connect(lambda: self.slider_pitch.setValue(0))
        p_row.addWidget(btn_reset_pitch)
        pg_layout.addLayout(p_row)

        ctrl_layout.addWidget(pitch_group)

        # 2. Robotic & Modulation Group
        robot_group = QGroupBox("Robotic Vocoder & Ring Modulation")
        rg_layout = QVBoxLayout(robot_group)

        self.chk_robot = QCheckBox("Enable Robotic Modulation")
        self.chk_robot.toggled.connect(self._on_controls_changed)
        rg_layout.addWidget(self.chk_robot)

        rf_row = QHBoxLayout()
        rf_row.addWidget(QLabel("Modulation Frequency:"))
        self.slider_rfreq = QSlider(Qt.Orientation.Horizontal)
        self.slider_rfreq.setRange(50, 500)
        self.slider_rfreq.setValue(150)
        self.slider_rfreq.valueChanged.connect(self._on_controls_changed)
        rf_row.addWidget(self.slider_rfreq)
        self.lbl_rfreq = QLabel("150 Hz")
        self.lbl_rfreq.setFixedWidth(55)
        rf_row.addWidget(self.lbl_rfreq)
        rg_layout.addLayout(rf_row)

        rm_row = QHBoxLayout()
        rm_row.addWidget(QLabel("Robot Mix:"))
        self.slider_rmix = QSlider(Qt.Orientation.Horizontal)
        self.slider_rmix.setRange(0, 100)
        self.slider_rmix.setValue(75)
        self.slider_rmix.valueChanged.connect(self._on_controls_changed)
        rm_row.addWidget(self.slider_rmix)
        self.lbl_rmix = QLabel("75%")
        self.lbl_rmix.setFixedWidth(55)
        rm_row.addWidget(self.lbl_rmix)
        rg_layout.addLayout(rm_row)

        ctrl_layout.addWidget(robot_group)

        # 3. Spatial & Filter Effects Group
        fx_group = QGroupBox("Spatial & Filter Effects")
        fx_layout = QVBoxLayout(fx_group)

        self.chk_radio = QCheckBox("Walkie-Talkie / Radio Bandpass Filter")
        self.chk_radio.toggled.connect(self._on_controls_changed)
        fx_layout.addWidget(self.chk_radio)

        dist_row = QHBoxLayout()
        self.chk_dist = QCheckBox("Distortion Drive:")
        self.chk_dist.toggled.connect(self._on_controls_changed)
        dist_row.addWidget(self.chk_dist)

        self.slider_dist = QSlider(Qt.Orientation.Horizontal)
        self.slider_dist.setRange(0, 100)
        self.slider_dist.setValue(0)
        self.slider_dist.valueChanged.connect(self._on_controls_changed)
        dist_row.addWidget(self.slider_dist)
        self.lbl_dist = QLabel("0%")
        self.lbl_dist.setFixedWidth(55)
        dist_row.addWidget(self.lbl_dist)
        fx_layout.addLayout(dist_row)

        rev_row = QHBoxLayout()
        self.chk_reverb = QCheckBox("Cathedral Reverb:")
        self.chk_reverb.toggled.connect(self._on_controls_changed)
        rev_row.addWidget(self.chk_reverb)

        self.slider_rev_wet = QSlider(Qt.Orientation.Horizontal)
        self.slider_rev_wet.setRange(0, 100)
        self.slider_rev_wet.setValue(0)
        self.slider_rev_wet.valueChanged.connect(self._on_controls_changed)
        rev_row.addWidget(self.slider_rev_wet)
        self.lbl_rev = QLabel("0%")
        self.lbl_rev.setFixedWidth(55)
        rev_row.addWidget(self.lbl_rev)
        fx_layout.addLayout(rev_row)

        chorus_row = QHBoxLayout()
        self.chk_chorus = QCheckBox("Spatial Chorus:")
        self.chk_chorus.toggled.connect(self._on_controls_changed)
        chorus_row.addWidget(self.chk_chorus)

        self.slider_chorus = QSlider(Qt.Orientation.Horizontal)
        self.slider_chorus.setRange(0, 100)
        self.slider_chorus.setValue(0)
        self.slider_chorus.valueChanged.connect(self._on_controls_changed)
        chorus_row.addWidget(self.slider_chorus)
        self.lbl_chorus = QLabel("0%")
        self.lbl_chorus.setFixedWidth(55)
        chorus_row.addWidget(self.lbl_chorus)
        fx_layout.addLayout(chorus_row)

        gate_row = QHBoxLayout()
        self.chk_gate = QCheckBox("Noise Gate:")
        self.chk_gate.setChecked(False)
        self.chk_gate.toggled.connect(self._on_controls_changed)
        gate_row.addWidget(self.chk_gate)

        self.slider_gate = QSlider(Qt.Orientation.Horizontal)
        self.slider_gate.setRange(-80, -30)
        self.slider_gate.setValue(-65)
        self.slider_gate.valueChanged.connect(self._on_controls_changed)
        gate_row.addWidget(self.slider_gate)
        self.lbl_gate = QLabel("-65 dB")
        self.lbl_gate.setFixedWidth(55)
        gate_row.addWidget(self.lbl_gate)
        fx_layout.addLayout(gate_row)

        ctrl_layout.addWidget(fx_group)

        scroll.setWidget(controls_container)
        layout.addWidget(scroll, stretch=1)

        return view

    # ==========================================
    # NAVIGATION & EVENT HANDLERS
    # ==========================================
    def _open_settings_page(self):
        """Switches to the Settings & DSP editor view."""
        self._refresh_presets_combo(select_name=self.active_preset)
        self.stack.setCurrentIndex(1)

    def _close_settings_page(self):
        """Switches back to the main presets view."""
        self._rebuild_main_preset_cards()
        self.stack.setCurrentIndex(0)

    def _refresh_presets_combo(self, select_name: Optional[str] = None):
        """Refreshes the presets dropdown on the settings page."""
        self.combo_edit_preset.blockSignals(True)
        self.combo_edit_preset.clear()

        all_presets = self.presets
        for name in all_presets:
            self.combo_edit_preset.addItem(name)

        if select_name and select_name in all_presets:
            self.combo_edit_preset.setCurrentText(select_name)
        else:
            self.combo_edit_preset.setCurrentText(self.active_preset)

        self.combo_edit_preset.blockSignals(False)
        self._update_delete_button_state()

    def _on_preset_combo_selected(self, preset_name: str):
        if not preset_name or self._is_loading_preset:
            return
        self.apply_preset(preset_name)
        self._update_delete_button_state()

    def _update_delete_button_state(self):
        current = self.combo_edit_preset.currentText()
        if current in DEFAULT_PRESETS:
            self.btn_delete_preset.setEnabled(False)
            self.btn_delete_preset.setToolTip(
                "Built-in presets (Clean, Deep Voice) cannot be deleted."
            )
        else:
            self.btn_delete_preset.setEnabled(True)
            self.btn_delete_preset.setToolTip(f"Delete custom preset '{current}'")

    def _clear_layout(self, layout):
        while layout.count():
            item = layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.deleteLater()
            elif item.layout() is not None:
                self._clear_layout(item.layout())

    def _get_current_ui_config(self) -> dict:
        """Snapshots current UI slider & effect states into a preset dict."""
        return {
            "pitch": self.slider_pitch.value() / 10.0,
            "robot": self.chk_robot.isChecked(),
            "rfreq": self.slider_rfreq.value(),
            "rmix": self.slider_rmix.value() / 100.0,
            "radio": self.chk_radio.isChecked(),
            "dist": self.chk_dist.isChecked(),
            "drive": self.slider_dist.value() / 100.0,
            "rev": self.chk_reverb.isChecked(),
            "rsize": 0.6,
            "rwet": self.slider_rev_wet.value() / 100.0,
            "chorus": self.chk_chorus.isChecked(),
            "cdepth": self.slider_chorus.value() / 100.0,
        }

    def create_new_preset(self):
        """Creates a new custom preset with current DSP slider settings."""
        name, ok = QInputDialog.getText(
            self,
            "New Custom Preset",
            "Enter a name for your new voice preset:",
        )
        if not ok or not name.strip():
            return

        name = name.strip()
        if name in DEFAULT_PRESETS:
            QMessageBox.warning(
                self,
                "Invalid Name",
                f"'{name}' is a built-in default preset name and cannot be overwritten.",
            )
            return

        if name in self.custom_presets:
            reply = QMessageBox.question(
                self,
                "Overwrite Preset",
                f"A custom preset named '{name}' already exists. Overwrite it?",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            )
            if reply != QMessageBox.StandardButton.Yes:
                return

        self.custom_presets[name] = self._get_current_ui_config()
        self.save_custom_presets()
        self.active_preset = name
        self._refresh_presets_combo(select_name=name)
        QMessageBox.information(
            self, "Preset Created", f"Custom preset '{name}' has been saved!"
        )

    def save_current_to_selected_preset(self):
        """Updates currently selected preset with current DSP slider settings."""
        preset_name = self.combo_edit_preset.currentText()
        if not preset_name:
            return

        if preset_name in DEFAULT_PRESETS:
            # Cannot directly overwrite built-in defaults; ask to create a new preset
            reply = QMessageBox.question(
                self,
                "Save as New Preset",
                f"'{preset_name}' is a built-in preset and cannot be modified directly.\n"
                f"Would you like to save your changes as a new custom preset?",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            )
            if reply == QMessageBox.StandardButton.Yes:
                self.create_new_preset()
            return

        self.custom_presets[preset_name] = self._get_current_ui_config()
        self.save_custom_presets()
        QMessageBox.information(
            self,
            "Preset Saved",
            f"Settings for custom preset '{preset_name}' updated successfully!",
        )

    def delete_selected_preset(self):
        """Deletes the currently selected custom preset."""
        preset_name = self.combo_edit_preset.currentText()
        if not preset_name or preset_name in DEFAULT_PRESETS:
            return

        reply = QMessageBox.question(
            self,
            "Delete Preset",
            f"Are you sure you want to delete custom preset '{preset_name}'?",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if reply != QMessageBox.StandardButton.Yes:
            return

        if preset_name in self.custom_presets:
            del self.custom_presets[preset_name]
            self.save_custom_presets()
            self.apply_preset("Clean")
            self._refresh_presets_combo(select_name="Clean")

    def _on_pitch_slider_changed(self, value):
        semitones = value / 10.0
        sign = "+" if semitones > 0 else ""
        self.lbl_pitch_val.setText(f"{sign}{semitones:.1f} st")
        self._on_controls_changed()

    def _on_controls_changed(self):
        # Update labels
        self.lbl_rfreq.setText(f"{self.slider_rfreq.value()} Hz")
        self.lbl_rmix.setText(f"{self.slider_rmix.value()}%")
        self.lbl_dist.setText(f"{self.slider_dist.value()}%")
        self.lbl_rev.setText(f"{self.slider_rev_wet.value()}%")
        self.lbl_chorus.setText(f"{self.slider_chorus.value()}%")
        self.lbl_gate.setText(f"{self.slider_gate.value()} dB")

        # Build options and apply to DSP in real time
        opts = DSPOptions(
            bypass=self.chk_bypass.isChecked(),
            noise_gate_enabled=self.chk_gate.isChecked(),
            noise_gate_threshold_db=float(self.slider_gate.value()),
            pitch_semitones=self.slider_pitch.value() / 10.0,
            robot_enabled=self.chk_robot.isChecked(),
            robot_freq=float(self.slider_rfreq.value()),
            robot_mix=self.slider_rmix.value() / 100.0,
            radio_enabled=self.chk_radio.isChecked(),
            distortion_enabled=self.chk_dist.isChecked(),
            distortion_drive=self.slider_dist.value() / 100.0,
            reverb_enabled=self.chk_reverb.isChecked(),
            reverb_room_size=0.6,
            reverb_wet=self.slider_rev_wet.value() / 100.0,
            chorus_enabled=self.chk_chorus.isChecked(),
            chorus_depth=self.slider_chorus.value() / 100.0,
        )
        self.dsp.update_options(opts)

    def apply_preset(self, preset_name: str):
        all_presets = self.presets
        if preset_name not in all_presets:
            return

        self._is_loading_preset = True
        self.active_preset = preset_name
        cfg = all_presets[preset_name]

        self.slider_pitch.setValue(int(cfg.get("pitch", 0.0) * 10))
        self.chk_robot.setChecked(cfg.get("robot", False))
        self.slider_rfreq.setValue(cfg.get("rfreq", 150))
        self.slider_rmix.setValue(int(cfg.get("rmix", 0.0) * 100))
        self.chk_radio.setChecked(cfg.get("radio", False))
        self.chk_dist.setChecked(cfg.get("dist", False))
        self.slider_dist.setValue(int(cfg.get("drive", 0.0) * 100))
        self.chk_reverb.setChecked(cfg.get("rev", False))
        self.slider_rev_wet.setValue(int(cfg.get("rwet", 0.0) * 100))
        self.chk_chorus.setChecked(cfg.get("chorus", False))
        self.slider_chorus.setValue(int(cfg.get("cdepth", 0.0) * 100))
        self.chk_bypass.setChecked(False)

        self._highlight_active_preset(preset_name)
        self._on_controls_changed()
        self._is_loading_preset = False
        self.preset_changed.emit(preset_name)

