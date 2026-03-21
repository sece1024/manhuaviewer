"""LRUCache 单元测试"""
from manhuaviewer.preload import LRUCache


class TestLRUCache:

    def test_put_and_get(self):
        cache = LRUCache(max_size=5)
        cache.put(0, "img0")
        cache.put(1, "img1")
        assert cache.get(0) == "img0"
        assert cache.get(1) == "img1"

    def test_get_missing_returns_none(self):
        cache = LRUCache(max_size=5)
        assert cache.get(99) is None

    def test_eviction(self):
        cache = LRUCache(max_size=3)
        cache.put(0, "a")
        cache.put(1, "b")
        cache.put(2, "c")
        cache.put(3, "d")  # should evict key 0

        assert cache.get(0) is None
        assert cache.get(1) == "b"
        assert cache.get(3) == "d"
        assert len(cache) == 3

    def test_access_refreshes_entry(self):
        cache = LRUCache(max_size=3)
        cache.put(0, "a")
        cache.put(1, "b")
        cache.put(2, "c")

        # Access key 0 to refresh it
        cache.get(0)

        cache.put(3, "d")  # should evict key 1 (least recently used)
        assert cache.get(0) == "a"  # still alive
        assert cache.get(1) is None  # evicted

    def test_clear(self):
        cache = LRUCache(max_size=5)
        for i in range(5):
            cache.put(i, f"img{i}")
        cache.clear()
        assert len(cache) == 0
        assert cache.get(0) is None

    def test_contains(self):
        cache = LRUCache(max_size=5)
        cache.put(42, "value")
        assert 42 in cache
        assert 99 not in cache

    def test_overwrite(self):
        cache = LRUCache(max_size=5)
        cache.put(1, "old")
        cache.put(1, "new")
        assert cache.get(1) == "new"
        assert len(cache) == 1
