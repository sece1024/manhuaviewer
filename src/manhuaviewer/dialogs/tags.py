"""标签管理对话框"""
import os

from PyQt5.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QPushButton, QLabel,
    QListWidget, QListWidgetItem, QLineEdit, QGridLayout,
)
from PyQt5.QtCore import Qt
from PyQt5.QtGui import QColor

from manhuaviewer.data_store import TagManager


class TagDialog(QDialog):
    """标签管理对话框"""

    def __init__(self, folder: str, tag_manager: TagManager, parent=None):
        super().__init__(parent)
        self.folder = folder
        self.tag_manager = tag_manager
        self.setWindowTitle(f"标签管理 — {os.path.basename(folder)}")
        self.setMinimumWidth(350)

        layout = QVBoxLayout(self)

        # 当前标签
        layout.addWidget(QLabel("当前标签:"))
        self.tag_list = QListWidget()
        layout.addWidget(self.tag_list)

        # 添加标签
        add_layout = QHBoxLayout()
        self.new_tag_input = QLineEdit()
        self.new_tag_input.setPlaceholderText("输入新标签...")
        self.new_tag_input.returnPressed.connect(self._add_tag)
        btn_add = QPushButton("添加")
        btn_add.clicked.connect(self._add_tag)
        add_layout.addWidget(self.new_tag_input)
        add_layout.addWidget(btn_add)
        layout.addLayout(add_layout)

        # 全局标签快捷添加
        all_tags = tag_manager.get_all_tags()
        current_tags = set(tag_manager.get_tags_for_folder(folder))
        quick_tags = [t for t in all_tags if t not in current_tags]
        if quick_tags:
            layout.addWidget(QLabel("快速添加:"))
            quick_layout = QGridLayout()
            for i, tag in enumerate(quick_tags[:12]):
                btn = QPushButton(tag)
                btn.setStyleSheet(
                    f"background-color: {tag_manager.tag_colors.get(tag, '#888888')}; "
                    "color: white; border-radius: 3px; padding: 4px 10px;"
                )
                btn.clicked.connect(lambda checked, t=tag: self._quick_add(t))
                quick_layout.addWidget(btn, i // 4, i % 4)
            layout.addLayout(quick_layout)

        # 删除按钮
        btn_remove = QPushButton("移除选中标签")
        btn_remove.clicked.connect(self._remove_selected)
        layout.addWidget(btn_remove)

        # 关闭
        btn_close = QPushButton("关闭")
        btn_close.clicked.connect(self.accept)
        layout.addWidget(btn_close)

        self._refresh_list()

    def _refresh_list(self):
        self.tag_list.clear()
        tags = self.tag_manager.get_tags_for_folder(self.folder)
        for tag in tags:
            item = QListWidgetItem(tag)
            color = self.tag_manager.tag_colors.get(tag, "#888888")
            item.setForeground(QColor(color))
            self.tag_list.addItem(item)

    def _add_tag(self):
        tag = self.new_tag_input.text().strip()
        if tag:
            self.tag_manager.add_tag(self.folder, tag)
            self.new_tag_input.clear()
            self._refresh_list()

    def _quick_add(self, tag):
        self.tag_manager.add_tag(self.folder, tag)
        self._refresh_list()

    def _remove_selected(self):
        item = self.tag_list.currentItem()
        if item:
            self.tag_manager.remove_tag(self.folder, item.text())
            self._refresh_list()
