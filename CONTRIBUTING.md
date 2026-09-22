# 贡献指南

感谢你对 MangaViewer 的关注！本文档说明如何搭建开发环境、运行测试以及发布新版本。

## 环境要求

- **Node.js** >= 18
- **pnpm** >= 9
- **Rust** (stable) — 通过 [rustup](https://rustup.rs/) 安装
- macOS 额外要求：Xcode Command Line Tools
- Linux 额外要求（Debian/Ubuntu，[Tauri 2 官方前置依赖](https://v2.tauri.app/start/prerequisites/) + 打包用 patchelf）：
  ```bash
  sudo apt-get update
  sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf
  ```
  > 注意是 `libayatana-appindicator3-dev`（旧的 `libappindicator3-dev` 在新版 Ubuntu 上已不可用）。

## 快速开始

```bash
git clone https://github.com/sece1024/manhuaviewer.git
cd manhuaviewer
pnpm install
pnpm tauri dev        # 启动 Tauri 开发模式（热重载）
```

## 项目结构

| 目录 | 说明 |
|------|------|
| `src-tauri/` | Tauri + Rust 后端（Axum + rusqlite） |
| `frontend/` | React 19 前端（Vite；Jest 用 react-scripts） |

## 开发命令

```bash
# Tauri 桌面应用（推荐）
pnpm tauri dev                     # 开发模式
pnpm tauri build                   # 本地生产构建

# 前端测试
cd frontend && pnpm test           # React 前端测试
cd src-tauri && cargo test         # Rust 后端测试

# 代码检查
cd src-tauri && cargo fmt --check  # Rust 格式化检查
cd src-tauri && cargo clippy -- -D warnings  # Rust lint
```

## 提交规范

使用 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

```
feat: 新增 CBZ 归档功能
fix: 修复生产模式下图片加载失败
docs: 更新 README
ci: 添加 Rust clippy 检查
chore: 移除 Electron 相关代码
```

## CI 自动检查

每次推送和 PR 会自动运行以下检查（`.github/workflows/ci.yml`）：

| Job | 内容 |
|-----|------|
| **frontend** | `pnpm --filter manhuaviewer-frontend build`（编译 + ESLint）+ `cd frontend && pnpm test`（前端测试） |
| **rust** | `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test` |

请在提交前确保本地通过这些检查。

## 本地构建安装包

### macOS

```bash
# Apple Silicon (M1+)
pnpm tauri build --target aarch64-apple-darwin

# Intel
pnpm tauri build --target x86_64-apple-darwin
```

产物位于 `src-tauri/target/<target>/release/bundle/`，包含 `.dmg` 和 `.app`。

### Windows

```bash
pnpm tauri build
```

产物位于 `src-tauri/target/release/bundle/`，包含 `.msi` 安装包。

### Linux

需先安装系统依赖（见上方"环境要求"），然后：

```bash
pnpm tauri build
```

产物位于 `src-tauri/target/release/bundle/`，包含 `.deb` 和 `.AppImage`。

## 自动发布流程

项目通过 GitHub Actions 自动构建和发布（`.github/workflows/release.yml`）。

### 触发方式

推送一个 `v` 开头的 Git 标签即可触发。**推荐一键发版：**

```bash
# 一条命令完成：bump 三处版本号 → pnpm changelog → commit → tag → push
./scripts/release.sh 3.5.4
#   -y        免交互确认
#   --no-push 只做到本地 commit + tag，不推送（稍后手动 git push origin main v3.5.4）
# 内置防呆：必须在 main、工作区干净（src-tauri/gen/schemas/ 固有改动豁免）、
#           git-cliff 已安装、目标 tag 不存在、版本号与当前不同
# 脚本只 stage 版本三文件 + CHANGELOG.md，不会用 git add -A 卷入游离改动
```

手动分步（等价于脚本内部步骤）：

```bash
# 1. 同步三处版本号（package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml）
./scripts/bump-version.sh 3.5.4
# 注意：脚本用的是 macOS 的 `sed -i ''` 语法，Linux 上需手动改用 `sed -i`
# frontend/package.json 的版本（2.0.0）是独立的，脚本不会动它——不要"顺手"同步

# 2. 重新生成 CHANGELOG.md（git-cliff，配置见 cliff.toml；需先安装：brew install git-cliff）
pnpm changelog

# 3. 提交并打标签
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml CHANGELOG.md
git commit -m "chore: release v3.5.4"
git tag v3.5.4
git push origin main v3.5.4
```

### 构建矩阵

| 平台 | Runner | 产物 |
|------|--------|------|
| macOS ARM64 (Apple Silicon) | `macos-latest` | `.dmg` |
| Windows x86_64 | `windows-latest` | `.msi` |

> 其他平台需本地构建，暂未加入 CI。

### 发布流程

1. 推送标签后，GitHub Actions 自动在多个平台并行构建
2. 构建完成后创建 **草稿 Release**，所有安装包作为 Release Assets 上传
3. 前往 [GitHub Releases](https://github.com/sece1024/manhuaviewer/releases) 页面检查产物
4. 确认无误后，点击 **Publish release** 正式发布

### 注意事项

- 发布前务必同步三处版本号（`package.json`、`tauri.conf.json`、`Cargo.toml`），推荐用 `./scripts/bump-version.sh <x.y.z>` 一次改完
- `CHANGELOG.md` 由 git-cliff 生成（`pnpm changelog`），**不要手工编辑**；草稿 Release 的正文由 CI 用 `git-cliff --latest --strip header` 自动生成（配置见 `cliff.toml`）
- Release 默认为草稿状态，需要手动确认发布
- 构建使用 [tauri-apps/tauri-action@v0](https://github.com/tauri-apps/tauri-action)，配置详见 `release.yml`
- macOS 构建使用 **ad-hoc 签名**（`APPLE_SIGNING_IDENTITY: "-"`，免费、无需 Apple Developer 账号，也未经 Apple 公证），用户首次打开仍需右键 > 打开，或在"系统设置 > 隐私与安全性"中允许

## 日志与启动问题排查

应用启动时会在数据目录下自动生成按天滚动的日志文件（保留最近 7 天）：

```
<data_dir>/logs/manhuaviewer.log.YYYY-MM-DD
```

其中 `<data_dir>` 默认路径：

| 平台 | 默认路径 |
|------|----------|
| macOS | `~/Library/Application Support/MangaViewer/data` |
| Windows | `%APPDATA%\MangaViewer\data` |
| Linux | `~/.local/share/MangaViewer/data` |

也可以通过 `DATA_DIR` 环境变量自定义位置。

### "安装完成后打开没有反应"排查步骤

1. **查看日志文件**：按上表找到 `logs/manhuaviewer.log.*`，日志中会记录启动失败的具体原因（数据库初始化失败、端口绑定失败、panic 等）。
2. **端口冲突**：应用内嵌的 HTTP 服务默认监听 `127.0.0.1:5002`。若已有一个 MangaViewer 实例在后台残留运行，新启动的实例会绑定失败并弹出错误提示（而不是静默退出）。可在任务管理器中检查是否有残留的 `manhuaviewer.exe` 进程并结束它；正常情况下应用已内置单实例锁，重复启动会自动聚焦到已打开的窗口而不是再开一个实例。
3. **WebView2 运行时**：Windows 安装包默认使用联网下载 WebView2 Bootstrapper 的安装方式（`webviewInstallMode` 未显式配置，使用 Tauri 默认值）。如果安装时无网络连接或下载失败，安装程序可能"看似成功"但系统缺少 WebView2 运行时，导致窗口无法渲染。可手动安装 [Microsoft Edge WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/) 后重试。
4. **杀毒软件/安全策略拦截**：部分杀毒软件会静默拦截未签名的应用进程，可临时关闭或加入信任列表后重试。
5. 若以上步骤仍无法定位问题，请在提交 Issue 时附上 `manhuaviewer.log.*` 日志文件内容。

## 问题反馈

- 提交 [Issue](https://github.com/sece1024/manhuaviewer/issues) 描述问题
- 附上操作系统、应用版本和复现步骤
