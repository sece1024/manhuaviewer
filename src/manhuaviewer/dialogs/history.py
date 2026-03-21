"""阅读历史对话框"""
import os

from PyQt5.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QPushButton, QLabel,
    QLineEdit, QListWidget, QListWidgetItem, QComboBox, QMessageBox,
)
from PyQt5.QtCore import pyqtSignal, Qt

from manhuaviewer.data_store import ReadingHistory, TagManager


class HistoryDialog(QDialog):
    """阅读历史对话框"""

    folder_selected = pyqtSignal(str)

    def __init__(self, history: ReadingHistory, tag_manager: TagManager, parent=None):
        super().__init__(parent)
        self.history = history
        self.tag_manager = tag_manager
        self.setWindowTitle("阅读历史")
        self.setMinimumSize(600, 400)

        layout = QVBoxLayout(self)

        # 搜索框
        search_layout = QHBoxLayout()
        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("🔍 搜索文件夹名称或标签...")
        self.search_input.textChanged.connect(self._filter_list)
        search_layout.addWidget(self.search_input)
        layout.addLayout(search_layout)

        # 标签过滤
        tag_layout = QHBoxLayout()
        tag_layout.addWidget(QLabel("标签筛选:"))
        self.tag_combo = QComboBox()
        self.tag_combo.addItem("全部")
        self.tag_combo.addItems(tag_manager.get_all_tags())
        self.tag_combo.currentTextChanged.connect(self._filter_list)
        tag_layout.addWidget(self.tag_combo)
        tag_layout.addStretch()
        layout.addLayout(tag_layout)

        # 列表
        self.list_widget = QListWidget()
        self.list_widget.itemDoubleClicked.connect(self._on_double_click)
        layout.addWidget(self.list_widget)

        # 按钮
        btn_layout = QHBoxLayout()
        btn_open = QPushButton("打开")
        btn_open.clicked.connect(self._open_selected)
        btn_delete = QPushButton("删除记录")
        btn_delete.clicked.connect(self._delete_selected)
        btn_clear = QPushButton("清空历史")
        btn_clear.clicked.connect(self._clear_all)
        btn_layout.addWidget(btn_open)
        btn_layout.addWidget(btn_delete)
        btn_layout.addStretch()
        btn_layout.addWidget(btn_clear)
        layout.addLayout(btn_layout)

        self._load_list()

    def _load_list(self):
        self.list_widget.clear()
        entries = self.history.get_all_history()
        for entry in entries:
            folder = entry["folder"]
            page = entry["page_index"] + 1
            total = entry["total_pages"]
            tags = self.tag_manager.get_tags_for_folder(folder)
            tag_str = " ".join(f"[{t}]" for t in tags) if tags else ""
            name = os.path.basename(folder) or folder
            display = f"{name}  —  第 {page}/{total} 页  {tag_str}"
            item = QListWidgetItem(display)
            item.setData(Qt.UserRole, folder)
            self.list_widget.addItem(item)

    def _filter_list(self):
        keyword = self.search_input.text().lower()
        tag_filter = self.tag_combo.currentText()

        for i in range(self.list_widget.count()):
            item = self.list_widget.item(i)
            folder = item.data(Qt.UserRole)
            text = item.text().lower()

            match_keyword = not keyword or keyword in text
            match_tag = tag_filter == "全部" or tag_filter in self.tag_manager.get_tags_for_folder(folder)

            item.setHidden(not (match_keyword and match_tag))

    def _get_selected_folder(self):
        item = self.list_widget.currentItem()
        if item:
            return item.data(Qt.UserRole)
        return None

    def _on_double_click(self, item):
        folder = item.data(Qt.UserRole)
        if folder:
            self.folder_selected.emit(folder)
            self.accept()

    def _open_selected(self):
        folder = self._get_selected_folder()
        if folder:
            self.folder_selected.emit(folder)
            self.accept()

    def _delete_selected(self):
        folder = self._get_selected_folder()
        if folder:
            self.history.remove_entry(folder)
            self._load_list()

    def _clear_all(self):
        reply = QMessageBox.question(
            self, "确认", "确定要清空所有阅读历史吗？",
            QMessageBox.Yes | QMessageBox.No
        )
        if reply == QMessageBox.Yes:
            self.history.clear()
            self._load_list()
