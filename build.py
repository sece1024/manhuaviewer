import PyInstaller.__main__
import os

# 获取当前目录
current_dir = os.path.dirname(os.path.abspath(__file__))

# 定义打包参数
params = [
    'comic_viewer_enhance.py',  # 主程序文件
    '--name=漫画浏览器',  # 生成的exe名称
    '--onefile',  # 打包成单个exe文件
    '--windowed',  # 不显示控制台窗口
    '--icon=NONE',  # 可以在这里指定图标文件路径
    '--add-data=README.md;.',  # 添加其他文件
    '--clean',  # 清理临时文件
    '--noconfirm',  # 不询问确认
]

# 执行打包
PyInstaller.__main__.run(params) 