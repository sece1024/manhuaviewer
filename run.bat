@echo off
REM 漫画浏览器 Windows 启动脚本
REM 用法: 双击 run.bat 或在命令行运行

if not exist ".venv\Scripts\python.exe" (
    echo 正在创建虚拟环境...
    uv venv
    echo 正在安装依赖...
    uv pip install -e .
)

.venv\Scripts\python.exe main.py
