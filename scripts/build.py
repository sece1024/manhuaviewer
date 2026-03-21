"""PyInstaller 打包脚本 - 支持 macOS / Windows / Linux"""
import PyInstaller.__main__
import os
import sys

current_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(current_dir)
main_script = os.path.join(project_root, "src", "manhuaviewer", "viewer.py")

# 路径分隔符: Windows 用分号，macOS/Linux 用冒号
sep = ";" if sys.platform == "win32" else ":"

params = [
    main_script,
    "--name=漫画浏览器",
    "--onefile",
    "--windowed",
    "--icon=NONE",
    f"--add-data=README.md{sep}.",
    "--paths=" + os.path.join(project_root, "src"),
    "--clean",
    "--noconfirm",
]

# macOS 特殊处理: 生成 .app bundle
if sys.platform == "darwin":
    params.append("--osx-bundle-identifier=com.manhuaviewer.app")

PyInstaller.__main__.run(params)
