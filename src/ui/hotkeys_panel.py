from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import (
    QFrame,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)
from ..input.hotkeys import HotkeyManager
from ..soundboard.manager import SoundboardManager


class HotkeysPanel(QWidget):
    """Global Hotkey Configuration & Permissions Management Panel."""

    def __init__(
        self,
        hotkey_mgr: HotkeyManager,
        soundboard_mgr: SoundboardManager,
        parent=None,
    ):
        super().__init__(parent)
        self.hotkey_mgr = hotkey_mgr
        self.soundboard_mgr = soundboard_mgr

        self.init_ui()

    def init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 16, 16, 16)
        layout.setSpacing(16)

        # 1. System Status / Wayland Permissions Card
        perm_group = QGroupBox("Wayland & Linux Global Shortcut Status")
        pg_layout = QVBoxLayout(perm_group)

        has_perm = self.hotkey_mgr.check_permissions()
        if has_perm:
            status_html = (
                "<b style='color: #00E676;'>✓ Global Input Access Active (/dev/input)</b><br>"
                "<span style='color: #CAD5E2;'>Hotkeys will work globally across games, Discord, and background windows.</span>"
            )
        else:
            status_html = (
                "<b style='color: #FFD600;'>⚠ Direct /dev/input Access Not Detected</b><br>"
                "<span style='color: #CAD5E2;'>For background global hotkeys on Wayland outside the window, add your user to the <code>input</code> group:</span><br>"
                "<code style='color: #00E5FF; background-color: #1B1E2E; padding: 2px 6px;'>sudo usermod -aG input $USER</code> "
                "<span style='color: #8F9CAE;'>(Requires log out / log in once).</span>"
            )

        self.lbl_perm_status = QLabel(status_html)
        self.lbl_perm_status.setTextFormat(Qt.TextFormat.RichText)
        pg_layout.addWidget(self.lbl_perm_status)

        layout.addWidget(perm_group)

        # 2. System Hotkeys Table
        table_group = QGroupBox("Global Action Shortcuts")
        tg_layout = QVBoxLayout(table_group)

        self.table = QTableWidget(4, 2)
        self.table.setHorizontalHeaderLabels(["Action", "Assigned Hotkey"])
        self.table.horizontalHeader().setSectionResizeMode(
            0, QHeaderView.ResizeMode.Stretch
        )
        self.table.horizontalHeader().setSectionResizeMode(
            1, QHeaderView.ResizeMode.ResizeToContents
        )

        actions = [
            ("Mute Microphone", "F9"),
            ("Bypass All DSP Effects", "F10"),
            ("Stop All Sounds (Panic)", "F11"),
            ("Toggle 'Hear Myself' (Loopback)", "F8"),
        ]

        for row, (act_name, default_key) in enumerate(actions):
            self.table.setItem(row, 0, QTableWidgetItem(act_name))
            key_item = QTableWidgetItem(default_key)
            key_item.setTextAlignment(Qt.AlignmentFlag.AlignCenter)
            self.table.setItem(row, 1, key_item)

        tg_layout.addWidget(self.table)
        layout.addWidget(table_group)

        layout.addStretch()
