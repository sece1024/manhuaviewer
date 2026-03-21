#!/bin/bash
# 漫画浏览器启动脚本 (macOS / Linux)
# 用法: bash run.sh

set -e

if [ ! -f ".venv/bin/python" ]; then
    echo "正在创建虚拟环境..."
    uv venv
    echo "正在安装依赖..."
    uv pip install -e .
fi

.venv/bin/python main.py
