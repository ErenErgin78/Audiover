"""
Modern Dark & Neon Theme Stylesheet for Audiover
"""

MODERN_STYLE = """
/* Global Window & Fonts */
QWidget {
    background-color: #0F111A;
    color: #E0E6ED;
    font-family: 'Segoe UI', 'Inter', 'Ubuntu', 'DejaVu Sans', sans-serif;
    font-size: 13px;
}

/* Sidebar Navigation */
QListWidget#navSidebar {
    background-color: #161824;
    border: none;
    border-right: 1px solid #23263B;
    outline: none;
    padding-top: 15px;
}

QListWidget#navSidebar::item {
    height: 48px;
    padding-left: 16px;
    margin: 4px 10px;
    border-radius: 8px;
    color: #8F9CAE;
    font-size: 14px;
    font-weight: 500;
}

QListWidget#navSidebar::item:hover {
    background-color: #1E2235;
    color: #FFFFFF;
}

QListWidget#navSidebar::item:selected {
    background-color: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #7C4DFF, stop:1 #00E5FF);
    color: #FFFFFF;
    font-weight: bold;
}

/* Cards & Group Boxes */
QGroupBox {
    background-color: #181A27;
    border: 1px solid #282C40;
    border-radius: 12px;
    margin-top: 18px;
    padding-top: 22px;
    font-size: 13px;
    font-weight: bold;
    color: #00E5FF;
}

QGroupBox::title {
    subcontrol-origin: margin;
    subcontrol-position: top left;
    padding: 2px 10px;
    background-color: #181A27;
    border-radius: 4px;
}

QFrame.cardFrame {
    background-color: #181A27;
    border: 1px solid #282C40;
    border-radius: 12px;
    padding: 12px;
}

/* Buttons */
QPushButton {
    background-color: #23273C;
    border: 1px solid #363B59;
    border-radius: 8px;
    color: #E2E8F0;
    padding: 8px 16px;
    font-weight: 600;
}

QPushButton:hover {
    background-color: #2D324D;
    border-color: #00E5FF;
    color: #FFFFFF;
}

QPushButton:pressed {
    background-color: #1B1E2E;
}

QPushButton:disabled {
    background-color: #141622;
    border-color: #1D2030;
    color: #4A5168;
}

/* Accent Buttons */
QPushButton#primaryBtn {
    background-color: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #7C4DFF, stop:1 #00E5FF);
    border: none;
    color: #FFFFFF;
    font-size: 14px;
    font-weight: bold;
}

QPushButton#primaryBtn:hover {
    background-color: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #8E65FF, stop:1 #33ECFF);
}

QPushButton#panicBtn {
    background-color: #FF1744;
    border: 1px solid #FF5252;
    color: #FFFFFF;
    font-weight: bold;
}

QPushButton#panicBtn:hover {
    background-color: #FF5252;
}

QPushButton#activePresetBtn {
    background-color: #7C4DFF;
    border: 1px solid #B388FF;
    color: #FFFFFF;
    font-weight: bold;
}

/* Sliders */
QSlider::groove:horizontal {
    border: none;
    height: 6px;
    background: #23273C;
    border-radius: 3px;
}

QSlider::sub-page:horizontal {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #7C4DFF, stop:1 #00E5FF);
    border-radius: 3px;
}

QSlider::handle:horizontal {
    background: #FFFFFF;
    border: 2px solid #00E5FF;
    width: 16px;
    margin-top: -5px;
    margin-bottom: -5px;
    border-radius: 8px;
}

QSlider::handle:horizontal:hover {
    background: #00E5FF;
    border-color: #FFFFFF;
}

/* Progress Bars / VU Meters */
QProgressBar {
    background-color: #161824;
    border: 1px solid #282C40;
    border-radius: 4px;
    text-align: center;
    color: #8F9CAE;
    font-size: 10px;
}

QProgressBar::chunk {
    background-color: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #00E676, stop:0.75 #FFD600, stop:0.95 #FF1744);
    border-radius: 3px;
}

/* Combo Boxes & Inputs */
QComboBox, QLineEdit {
    background-color: #1B1E2E;
    border: 1px solid #2F354F;
    border-radius: 8px;
    padding: 6px 12px;
    color: #E2E8F0;
}

QComboBox:hover, QLineEdit:focus {
    border-color: #00E5FF;
}

QComboBox::drop-down {
    border: none;
    width: 24px;
}

QComboBox QAbstractItemView {
    background-color: #181A27;
    border: 1px solid #2F354F;
    selection-background-color: #7C4DFF;
    color: #FFFFFF;
    padding: 4px;
}

/* Checkboxes */
QCheckBox {
    spacing: 8px;
    color: #CAD5E2;
    font-weight: 500;
}

QCheckBox::indicator {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid #363B59;
    background-color: #1B1E2E;
}

QCheckBox::indicator:checked {
    background-color: #00E5FF;
    border-color: #00E5FF;
    image: none;
}

/* ScrollBars */
QScrollBar:vertical {
    border: none;
    background: #0F111A;
    width: 8px;
    margin: 0;
}

QScrollBar::handle:vertical {
    background: #282C40;
    min-height: 20px;
    border-radius: 4px;
}

QScrollBar::handle:vertical:hover {
    background: #3E4463;
}

QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    height: 0px;
}
"""
