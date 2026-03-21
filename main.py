"""漫画浏览器 - 入口点"""
import sys
from comic_viewer_enhance import ComicViewer
from PyQt5.QtWidgets import QApplication


def main():
    app = QApplication(sys.argv)
    viewer = ComicViewer()
    viewer.show()
    sys.exit(app.exec_())


if __name__ == "__main__":
    main()
