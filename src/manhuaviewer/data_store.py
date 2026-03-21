"""
数据持久化层 - 阅读历史与标签管理
使用 JSON 文件存储，线程安全
"""
import json
import os
import time
from pathlib import Path
from typing import Optional


def _get_data_dir() -> Path:
    """获取数据存储目录"""
    if os.name == "nt":
        base = os.environ.get("APPDATA", os.path.expanduser("~"))
    else:
        base = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    d = Path(base) / "ManhuaViewer"
    d.mkdir(parents=True, exist_ok=True)
    return d


def _get_history_path() -> Path:
    return _get_data_dir() / "history.json"


def _get_tags_path() -> Path:
    return _get_data_dir() / "tags.json"


def _load_json(path: Path) -> dict:
    if path.exists():
        try:
            with open(path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return {}
    return {}


def _save_json(path: Path, data: dict):
    """安全写入 JSON — 先写临时文件再原子替换，防止写入中断导致数据丢失"""
    import tempfile
    dir_name = str(path.parent)
    try:
        fd, tmp_path = tempfile.mkstemp(dir=dir_name, suffix=".tmp")
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(data, f, ensure_ascii=False, indent=2)
            os.replace(tmp_path, str(path))
        except Exception:
            os.unlink(tmp_path)
            raise
    except Exception:
        # 如果 mkstemp 失败，回退到简单写入
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)


# ── 阅读历史 ────────────────────────────────────────────────────

class ReadingHistory:
    """管理每个文件夹的阅读进度"""

    def __init__(self, path: Optional[Path] = None):
        self._path = path or _get_history_path()
        self._data: dict = _load_json(self._path)

    def save_progress(self, folder: str, page_index: int, total_pages: int):
        """保存阅读进度"""
        folder = os.path.abspath(folder)
        self._data[folder] = {
            "page_index": page_index,
            "total_pages": total_pages,
            "timestamp": time.time(),
        }
        _save_json(self._path, self._data)

    def get_progress(self, folder: str) -> Optional[dict]:
        """获取阅读进度，返回 None 表示无记录"""
        folder = os.path.abspath(folder)
        return self._data.get(folder)

    def get_all_history(self) -> list[dict]:
        """获取所有历史记录，按时间倒序"""
        entries = []
        for folder, info in self._data.items():
            if os.path.isdir(folder):
                entries.append({
                    "folder": folder,
                    "page_index": info.get("page_index", 0),
                    "total_pages": info.get("total_pages", 0),
                    "timestamp": info.get("timestamp", 0),
                })
        entries.sort(key=lambda x: x["timestamp"], reverse=True)
        return entries

    def remove_entry(self, folder: str):
        """删除某条记录"""
        folder = os.path.abspath(folder)
        self._data.pop(folder, None)
        _save_json(self._path, self._data)

    def clear(self):
        """清除所有历史"""
        self._data.clear()
        _save_json(self._path, self._data)


# ── 标签管理 ────────────────────────────────────────────────────

class TagManager:
    """管理漫画文件夹的标签"""

    def __init__(self, path: Optional[Path] = None):
        self._path = path or _get_tags_path()
        self._data: dict = _load_json(self._path)
        # folder_tags: {folder_path: [tag1, tag2, ...]}
        # tag_colors:  {tag_name: "#hexcolor"}
        if "folder_tags" not in self._data:
            self._data["folder_tags"] = {}
        if "tag_colors" not in self._data:
            self._data["tag_colors"] = {}

    @property
    def folder_tags(self) -> dict[str, list[str]]:
        return self._data["folder_tags"]

    @property
    def tag_colors(self) -> dict[str, str]:
        return self._data["tag_colors"]

    def get_all_tags(self) -> list[str]:
        """获取所有已使用的标签"""
        tags = set()
        for tag_list in self._data["folder_tags"].values():
            tags.update(tag_list)
        return sorted(tags)

    def get_tags_for_folder(self, folder: str) -> list[str]:
        """获取文件夹的标签"""
        folder = os.path.abspath(folder)
        return list(self._data["folder_tags"].get(folder, []))

    def add_tag(self, folder: str, tag: str, color: str = "#4a86e8"):
        """给文件夹添加标签"""
        folder = os.path.abspath(folder)
        tag = tag.strip()
        if not tag:
            return
        if folder not in self._data["folder_tags"]:
            self._data["folder_tags"][folder] = []
        if tag not in self._data["folder_tags"][folder]:
            self._data["folder_tags"][folder].append(tag)
        if tag not in self._data["tag_colors"]:
            self._data["tag_colors"][tag] = color
        _save_json(self._path, self._data)

    def remove_tag(self, folder: str, tag: str):
        """移除文件夹的标签"""
        folder = os.path.abspath(folder)
        if folder in self._data["folder_tags"]:
            self._data["folder_tags"][folder] = [
                t for t in self._data["folder_tags"][folder] if t != tag
            ]
        _save_json(self._path, self._data)

    def rename_tag(self, old_name: str, new_name: str):
        """重命名标签"""
        new_name = new_name.strip()
        if not new_name or old_name == new_name:
            return
        for folder in self._data["folder_tags"]:
            self._data["folder_tags"][folder] = [
                new_name if t == old_name else t
                for t in self._data["folder_tags"][folder]
            ]
        if old_name in self._data["tag_colors"]:
            self._data["tag_colors"][new_name] = self._data["tag_colors"].pop(old_name)
        _save_json(self._path, self._data)

    def delete_tag(self, tag: str):
        """全局删除标签"""
        for folder in self._data["folder_tags"]:
            self._data["folder_tags"][folder] = [
                t for t in self._data["folder_tags"][folder] if t != tag
            ]
        self._data["tag_colors"].pop(tag, None)
        _save_json(self._path, self._data)

    def get_folders_by_tag(self, tag: str) -> list[str]:
        """获取拥有某标签的所有文件夹"""
        return [
            folder for folder, tags in self._data["folder_tags"].items()
            if tag in tags and os.path.isdir(folder)
        ]

    def set_tag_color(self, tag: str, color: str):
        """设置标签颜色"""
        self._data["tag_colors"][tag] = color
        _save_json(self._path, self._data)
