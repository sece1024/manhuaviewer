"""设置对话框 - 支持恢复已保存值 + 背景颜色生效"""
import logging

from PyQt5.QtWidgets import (
    QDialog, QFormLayout, QSlider, QPushButton,
    QColorDialog, QComboBox, QDialogButtonBox, QLabel,
)
from PyQt5.QtCore import Qt
from PyQt5.QtGui import QColor

from manhuaviewer.styles import THEMES

logger = logging.getLogger(__name__)


class SettingsDialog(QDialog):
    """设置对话框"""

    def __init__(self, parent=None, current_scale: float = 1.0,
                 current_theme: str = "浅色", bg_color: str = "#ffffff"):
        super().__init__(parent)
        self.setWindowTitle("设置")
        self.setMinimumWidth(300)

        self._bg_color = bg_color

        layout = QFormLayout(self)

        # 当前缩放值显示
        self.scale_label = QLabel(f"{int(current_scale * 100)}%")
        self.scale_slider = QSlider(Qt.Horizontal)
        self.scale_slider.setMinimum(50)
        self.scale_slider.setMaximum(200)
        self.scale_slider.setValue(int(current_scale * 100))
        self.scale_slider.setTickPosition(QSlider.TicksBelow)
        self.scale_slider.setTickInterval(10)
        self.scale_slider.valueChanged.connect(
            lambda v: self.scale_label.setText(f"{v}%")
        )
        layout.addRow("图片缩放:", self.scale_slider)
        layout.addRow("", self.scale_label)

        # 背景颜色
        self.bg_color_btn = QPushButton("选择颜色")
        self.bg_color_btn.setStyleSheet(
            f"background-color: {bg_color}; border: 1px solid #cccccc; border-radius: 4px; "
            "min-width: 60px; min-height: 24px;"
        )
        self.bg_color_btn.clicked.connect(self._choose_bg_color)
        layout.addRow("背景颜色:", self.bg_color_btn)

        # 主题
        self.theme_combo = QComboBox()
        self.theme_combo.addItems(list(THEMES.keys()))
        self.theme_combo.setCurrentText(current_theme)
        layout.addRow("主题:", self.theme_combo)

        button_box = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        button_box.accepted.connect(self.accept)
        button_box.rejected.connect(self.reject)
        layout.addRow(button_box)

    def _choose_bg_color(self):
        color = QColorDialog.getColor(QColor(self._bg_color), self, "选择背景颜色")
        if color.isValid():
            self._bg_color = color.name()
            self.bg_color_btn.setStyleSheet(
                f"background-color: {color.name()}; border: 1px solid #cccccc; border-radius: 4px; "
                "min-width: 60px; min-height: 24px;"
            )

    def get_bg_color(self) -> str:
        return self._bg_color
