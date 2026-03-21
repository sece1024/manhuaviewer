"""样式表与主题定义"""

STYLE_SHEET = """
    QMainWindow { background-color: #f0f0f0; }
    QPushButton {
        background-color: #4a86e8; color: white; border: none;
        padding: 8px 16px; border-radius: 4px; font-size: 14px;
    }
    QPushButton:hover { background-color: #3a76d8; }
    QPushButton:pressed { background-color: #2a66c8; }
    QPushButton:disabled { background-color: #a0a0a0; }
    QLabel { color: #333333; font-size: 14px; }
    QCheckBox { color: #333333; font-size: 14px; spacing: 8px; }
    QCheckBox::indicator { width: 18px; height: 18px; }
    QGraphicsView {
        background-color: #ffffff; border: 1px solid #dddddd; border-radius: 4px;
    }
    QMenuBar { background-color: #ffffff; border-bottom: 1px solid #dddddd; }
    QMenuBar::item { padding: 4px 8px; background-color: transparent; }
    QMenuBar::item:selected { background-color: #e0e0e0; }
    QMenu { background-color: #ffffff; border: 1px solid #dddddd; }
    QMenu::item { padding: 6px 20px; }
    QMenu::item:selected { background-color: #e0e0e0; }
    QProgressBar {
        border: 1px solid #dddddd; border-radius: 4px; text-align: center;
        height: 6px; background-color: #f0f0f0;
    }
    QProgressBar::chunk { background-color: #4a86e8; border-radius: 3px; }
"""

THEMES = {
    "浅色": "QMainWindow { background-color: #f0f0f0; } QGraphicsView { background-color: #ffffff; }",
    "深色": """
        QMainWindow { background-color: #2d2d2d; }
        QGraphicsView { background-color: #1a1a1a; }
        QLabel { color: #ffffff; }
        QCheckBox { color: #ffffff; }
    """,
    "护眼": "QMainWindow { background-color: #f0f7eb; } QGraphicsView { background-color: #ffffff; }",
}
