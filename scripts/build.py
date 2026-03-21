"""PyInstaller 打包脚本"""
import PyInstaller.__main__
import os

current_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(current_dir)
main_script = os.path.join(project_root, "src", "manhuaviewer", "viewer.py")

params = [
    main_script,
    "--name=漫画浏览器",
    "--onefile",
    "--windowed",
    "--icon=NONE",
    "--add-data=README.md;.",
    "--paths=" + os.path.join(project_root, "src"),
    "--clean",
    "--noconfirm",
]

PyInstaller.__main__.run(params)
