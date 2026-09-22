#!/usr/bin/env bash
# scripts/release.sh — 一键发版：bump → changelog → commit → tag → push
# 用法: ./scripts/release.sh <新版本号> [-y] [--check] [--no-push]
# 示例: ./scripts/release.sh 3.5.4        # 交互确认后推送（触发 release.yml 打包）
#       ./scripts/release.sh 3.5.4 -y     # 免确认
#       ./scripts/release.sh 3.5.4 --check   # 打 tag 前先跑 lint/fmt/build/测试，全绿才继续
#       ./scripts/release.sh 3.5.4 --no-push   # 只做到本地 commit + tag，不推送
#
# 防呆检查（全部通过才会改动任何文件）：
#   - 版本号格式 x.y.z，且与当前 package.json 版本不同
#   - 当前在 main 分支
#   - 工作区干净（src-tauri/gen/schemas/ 的固有改动除外，仓库约定不提交它）
#   - git-cliff 已安装（pnpm changelog 依赖）
#   - 目标 tag v<版本> 尚不存在
#   - --check 指定时：lint / fmt / build / 前端测试 / cargo test 全绿
#
# 注意：脚本只 stage 版本三文件 + CHANGELOG.md，绝不用 `git add -A`，
# 避免把 gen/schemas 等游离改动带进 release 提交。

set -euo pipefail

usage() {
  echo "用法: $0 <版本号> [-y] [--check] [--no-push]"
  echo "示例: $0 3.5.4 -y --check"
  exit 1
}

die() {
  echo "❌ $*" >&2
  exit 1
}

VERSION=""
ASSUME_YES=0
NO_PUSH=0
DO_CHECK=0
for arg in "$@"; do
  case "$arg" in
    -y | --yes) ASSUME_YES=1 ;;
    --no-push) NO_PUSH=1 ;;
    --check) DO_CHECK=1 ;;
    -h | --help) usage ;;
    *)
      [ -z "$VERSION" ] || usage
      VERSION="$arg"
      ;;
  esac
done
[ -n "$VERSION" ] || usage

# 校验版本号格式 (x.y.z)
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "版本号格式错误，应为 x.y.z（如 3.5.4）"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# ── 防呆检查（此时尚未改动任何文件）──

BRANCH="$(git branch --show-current)"
[ "$BRANCH" = "main" ] || die "当前在 $BRANCH 分支，发版必须在 main 上"

# 工作区必须干净；gen/schemas 的固有改动按仓库约定豁免
DIRTY="$(git status --porcelain | grep -v 'src-tauri/gen/schemas/' || true)"
if [ -n "$DIRTY" ]; then
  echo "❌ 工作区有未提交改动，请先提交或丢弃：" >&2
  echo "$DIRTY" >&2
  exit 1
fi

command -v git-cliff >/dev/null 2>&1 || die "未找到 git-cliff（pnpm changelog 依赖它）：brew install git-cliff"

CURRENT="$(node -p "require('./package.json').version")"
[ "$CURRENT" != "$VERSION" ] || die "版本号与当前相同（${CURRENT}），无需发版"

git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null && die "tag v$VERSION 已存在，可能已发过这个版"

# --check：本地验证门禁。放在确认之前——先花几分钟跑测试，
# 最后的 y/N 才是干净的"最后一步"；任何一步失败即中止，此时还没改动任何文件。
if [ "$DO_CHECK" = "1" ]; then
  echo "🔍 --check：运行验证套件（lint / fmt / build / 前端测试 / cargo test）..."
  if ! (
    pnpm lint &&
      pnpm format:check &&
      pnpm --filter manhuaviewer-frontend build &&
      (cd frontend && CI=true pnpm test) &&
      cargo test --manifest-path src-tauri/Cargo.toml
  ); then
    die "验证未通过，未做任何改动"
  fi
  echo "✅ 验证全绿"
  echo ""
fi

# ── 确认 ──
echo "📋 发版计划:"
echo "   版本: $CURRENT → $VERSION"
echo "   提交: package.json + src-tauri/tauri.conf.json + src-tauri/Cargo.toml + CHANGELOG.md"
echo "   标签: v$VERSION"
if [ "$NO_PUSH" = "1" ]; then
  echo "   推送: 否（--no-push，稍后手动推）"
else
  echo "   推送: 是（push 后 release.yml 自动打包）"
fi
echo ""

if [ "$ASSUME_YES" != "1" ]; then
  read -r -p "确认继续？[y/N] " ANSWER || ANSWER=""
  case "$ANSWER" in
    y | Y | yes | YES) ;;
    *) echo "已取消。"; exit 0 ;;
  esac
  echo ""
fi

# ── 执行 ──

# 1. 同步三处版本号（macOS sed 语法，Linux 需手动改用 sed -i）
./scripts/bump-version.sh "$VERSION"
echo ""

# 2. 重新生成 CHANGELOG.md（含自上个 tag 以来的提交；release 提交本身尚未生成，属预期）
pnpm changelog
echo "✅ CHANGELOG.md 已重新生成"

# 3. 提交（只 stage 版本三文件 + CHANGELOG.md）并打轻量 tag
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml CHANGELOG.md
git commit -m "chore: release v$VERSION"
git tag "v$VERSION"
echo "✅ 已提交并打标签 v$VERSION"

# 4. 推送
if [ "$NO_PUSH" = "1" ]; then
  echo ""
  echo "⏸  未推送（--no-push）。确认无误后手动执行："
  echo "   git push origin main v$VERSION"
else
  git push origin main "v$VERSION"
  echo ""
  echo "🚀 已推送。release.yml 开始打包（macOS .dmg + Windows .msi）："
  echo "   gh run list --workflow=release.yml --limit 1"
  echo "   打包完成后去 Releases 页面手动 Publish 草稿 Release"
fi
