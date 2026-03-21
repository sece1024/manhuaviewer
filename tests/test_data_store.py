"""data_store.py 单元测试"""
import json
import os
import tempfile
from pathlib import Path

import pytest

from data_store import ReadingHistory, TagManager


@pytest.fixture
def history_path(tmp_path):
    return tmp_path / "history.json"


@pytest.fixture
def tags_path(tmp_path):
    return tmp_path / "tags.json"


@pytest.fixture
def history(history_path):
    return ReadingHistory(path=history_path)


@pytest.fixture
def tag_manager(tags_path):
    return TagManager(path=tags_path)


# ── ReadingHistory 测试 ────────────────────────────────────────

class TestReadingHistory:

    def test_save_and_get_progress(self, history, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)

        history.save_progress(folder, page_index=5, total_pages=20)
        result = history.get_progress(folder)

        assert result is not None
        assert result["page_index"] == 5
        assert result["total_pages"] == 20
        assert "timestamp" in result

    def test_get_progress_returns_none_for_unknown(self, history):
        result = history.get_progress("/nonexistent/path")
        assert result is None

    def test_overwrite_progress(self, history, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)

        history.save_progress(folder, 3, 10)
        history.save_progress(folder, 7, 10)
        result = history.get_progress(folder)

        assert result["page_index"] == 7

    def test_get_all_history_sorted_by_time(self, history, tmp_path):
        folder1 = str(tmp_path / "comic1")
        folder2 = str(tmp_path / "comic2")
        os.makedirs(folder1)
        os.makedirs(folder2)

        history.save_progress(folder1, 1, 10)
        import time
        time.sleep(0.01)
        history.save_progress(folder2, 5, 10)

        all_hist = history.get_all_history()
        assert len(all_hist) == 2
        # 最新的在前
        assert all_hist[0]["folder"] == folder2
        assert all_hist[1]["folder"] == folder1

    def test_history_excludes_deleted_folders(self, history, tmp_path):
        folder = str(tmp_path / "will_delete")
        os.makedirs(folder)
        history.save_progress(folder, 1, 10)
        os.rmdir(folder)

        all_hist = history.get_all_history()
        assert len(all_hist) == 0

    def test_remove_entry(self, history, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)
        history.save_progress(folder, 1, 10)
        history.remove_entry(folder)

        assert history.get_progress(folder) is None

    def test_clear_all(self, history, tmp_path):
        for i in range(3):
            f = str(tmp_path / f"comic{i}")
            os.makedirs(f)
            history.save_progress(f, i, 10)

        history.clear()
        assert len(history.get_all_history()) == 0

    def test_persistence(self, tmp_path):
        path = tmp_path / "hist.json"
        h1 = ReadingHistory(path=path)
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)
        h1.save_progress(folder, 8, 15)

        # 重新加载验证持久化
        h2 = ReadingHistory(path=path)
        result = h2.get_progress(folder)
        assert result is not None
        assert result["page_index"] == 8

    def test_path_normalization(self, history, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)
        history.save_progress(folder + os.sep, 3, 10)

        result = history.get_progress(folder)
        assert result is not None
        assert result["page_index"] == 3


# ── TagManager 测试 ────────────────────────────────────────────

class TestTagManager:

    def test_add_and_get_tags(self, tag_manager, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)

        tag_manager.add_tag(folder, "热血")
        tag_manager.add_tag(folder, "少年")

        tags = tag_manager.get_tags_for_folder(folder)
        assert "热血" in tags
        assert "少年" in tags

    def test_add_duplicate_tag(self, tag_manager, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)

        tag_manager.add_tag(folder, "搞笑")
        tag_manager.add_tag(folder, "搞笑")

        tags = tag_manager.get_tags_for_folder(folder)
        assert tags.count("搞笑") == 1

    def test_remove_tag(self, tag_manager, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)

        tag_manager.add_tag(folder, "热血")
        tag_manager.remove_tag(folder, "热血")

        tags = tag_manager.get_tags_for_folder(folder)
        assert "热血" not in tags

    def test_get_all_tags(self, tag_manager, tmp_path):
        for name in ["comic1", "comic2"]:
            f = str(tmp_path / name)
            os.makedirs(f)
            tag_manager.add_tag(f, "热血")
            tag_manager.add_tag(f, "搞笑")

        tag_manager.add_tag(str(tmp_path / "comic1"), "悬疑")

        all_tags = tag_manager.get_all_tags()
        assert set(all_tags) == {"搞笑", "热血", "悬疑"}

    def test_rename_tag(self, tag_manager, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)
        tag_manager.add_tag(folder, "旧名字", color="#ff0000")

        tag_manager.rename_tag("旧名字", "新名字")

        tags = tag_manager.get_tags_for_folder(folder)
        assert "新名字" in tags
        assert "旧名字" not in tags
        assert tag_manager.tag_colors.get("新名字") == "#ff0000"

    def test_delete_tag_globally(self, tag_manager, tmp_path):
        for name in ["comic1", "comic2"]:
            f = str(tmp_path / name)
            os.makedirs(f)
            tag_manager.add_tag(f, "要删除")

        tag_manager.delete_tag("要删除")

        assert tag_manager.get_all_tags() == []

    def test_get_folders_by_tag(self, tag_manager, tmp_path):
        f1 = str(tmp_path / "comic1")
        f2 = str(tmp_path / "comic2")
        os.makedirs(f1)
        os.makedirs(f2)

        tag_manager.add_tag(f1, "热血")
        tag_manager.add_tag(f2, "热血")
        tag_manager.add_tag(f1, "搞笑")

        result = tag_manager.get_folders_by_tag("热血")
        assert len(result) == 2

        result = tag_manager.get_folders_by_tag("搞笑")
        assert len(result) == 1

    def test_tag_color(self, tag_manager, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)
        tag_manager.add_tag(folder, "测试", color="#123456")

        assert tag_manager.tag_colors["测试"] == "#123456"

        tag_manager.set_tag_color("测试", "#abcdef")
        assert tag_manager.tag_colors["测试"] == "#abcdef"

    def test_empty_tag_ignored(self, tag_manager, tmp_path):
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)

        tag_manager.add_tag(folder, "")
        tag_manager.add_tag(folder, "  ")

        assert tag_manager.get_tags_for_folder(folder) == []

    def test_persistence(self, tmp_path):
        path = tmp_path / "tags.json"
        tm1 = TagManager(path=path)
        folder = str(tmp_path / "comic1")
        os.makedirs(folder)
        tm1.add_tag(folder, "持久化测试")

        tm2 = TagManager(path=path)
        assert "持久化测试" in tm2.get_tags_for_folder(folder)
