"""跳转到指定页对话框"""
from PyQt5.QtWidgets import (
    QDialog, QFormLayout, QSlider, QDialogButtonBox, QLabel,
)
from PyQt5.QtCore import Qt


class JumpPageDialog(QDialog):
    """跳转到指定页"""

    def __init__(self, current: int, total: int, parent=None):
        super().__init__(parent)
        self.setWindowTitle("跳转到")
        layout = QFormLayout(self)

        self.page_spin = QSlider(Qt.Horizontal)
        self.page_spin.setMinimum(1)
        self.page_spin.setMaximum(total)
        self.page_spin.setValue(current + 1)
        self.page_spin.setTickPosition(QSlider.TicksBelow)

        self.page_label = QLabel(f"第 {current + 1} / {total} 页")
        self.page_spin.valueChanged.connect(
            lambda v: self.page_label.setText(f"第 {v} / {total} 页")
        )

        layout.addRow(self.page_label)
        layout.addRow(self.page_spin)

        btn_box = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        btn_box.accepted.connect(self.accept)
        btn_box.rejected.connect(self.reject)
        layout.addRow(btn_box)

    def get_page(self) -> int:
        return self.page_spin.value() - 1
