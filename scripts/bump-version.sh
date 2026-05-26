#!/usr/bin/env bash
# scripts/bump-version.sh — 统一修改项目版本号
# 用法: ./scripts/bump-version.sh <新版本号>
# 示例: ./scripts/bump-version.sh 3.1.0

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "用法: $0 <版本号>"
  echo "示例: $0 3.1.0"
  exit 1
fi

VERSION="$1"

# 校验版本号格式 (x.y.z)
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "❌ 版本号格式错误，应为 x.y.z（如 3.1.0）"
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "📦 将版本号统一修改为 $VERSION"
echo ""

# 1. package.json (root)
FILE="$ROOT/package.json"
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" "$FILE"
echo "✅ package.json → $VERSION"

# 2. src-tauri/tauri.conf.json
FILE="$ROOT/src-tauri/tauri.conf.json"
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" "$FILE"
echo "✅ src-tauri/tauri.conf.json → $VERSION"

# 3. src-tauri/Cargo.toml
FILE="$ROOT/src-tauri/Cargo.toml"
sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$FILE"
echo "✅ src-tauri/Cargo.toml → $VERSION"

echo ""
echo "🔍 验证结果:"
echo "   package.json:      $(grep '"version"' "$ROOT/package.json" | head -1 | xargs)"
echo "   tauri.conf.json:   $(grep '"version"' "$ROOT/src-tauri/tauri.conf.json" | head -1 | xargs)"
echo "   Cargo.toml:        $(grep '^version' "$ROOT/src-tauri/Cargo.toml" | xargs)"

echo ""
echo "📋 下一步:"
echo "   git add -A && git commit -m \"chore: release v$VERSION\""
echo "   git tag v$VERSION"
echo "   git push origin main --tags"
