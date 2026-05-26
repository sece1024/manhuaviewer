# 贡献指南

感谢你对 MangaViewer 的关注！本文档说明如何搭建开发环境、运行测试以及发布新版本。

## 环境要求

- **Node.js** >= 18
- **pnpm** >= 9
- **Rust** (stable) — 通过 [rustup](https://rustup.rs/) 安装
- macOS 额外要求：Xcode Command Line Tools
- Linux 额外要求：
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
  ```

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
| `frontend/` | React 19 前端（CRA） |
| `backend/` | Legacy Node.js 后端（Express + better-sqlite3） |

## 开发命令

```bash
# Tauri 桌面应用（推荐）
pnpm tauri dev                     # 开发模式
pnpm tauri build                   # 本地生产构建

# Web 模式（Legacy Node.js 后端）
pnpm start                         # 同时启动前后端

# 测试
cd backend && pnpm test            # Node.js 后端测试
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
| **frontend** | `pnpm build`（编译 + ESLint） |
| **backend** | Jest 单元测试 |
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

推送一个 `v` 开头的 Git 标签即可触发：

```bash
# 1. 确保版本号一致
#    - src-tauri/tauri.conf.json → "version"
#    - src-tauri/Cargo.toml → version
#    - package.json → version

# 2. 提交并打标签
git add -A
git commit -m "chore: release v3.1.0"
git tag v3.1.0
git push origin main --tags
```

### 构建矩阵

| 平台 | Runner | 产物 |
|------|--------|------|
| macOS ARM64 (Apple Silicon) | `macos-latest` | `.dmg` |
| Windows x86_64 | `windows-latest` | `.msi` |

> 其他平台需本地构建，暂未加入 CI。

### 发布流程

1. 推送标签后，GitHub Actions 自动在 4 个平台并行构建
2. 构建完成后创建 **草稿 Release**，所有安装包作为 Release Assets 上传
3. 前往 [GitHub Releases](https://github.com/sece1024/manhuaviewer/releases) 页面检查产物
4. 确认无误后，点击 **Publish release** 正式发布

### 注意事项

- 发布前务必同步三处版本号（`tauri.conf.json`、`Cargo.toml`、`package.json`）
- Release 默认为草稿状态，需要手动确认发布
- 构建使用 [tauri-apps/tauri-action@v1](https://github.com/nicegui-org/tauri-action)，配置详见 `release.yml`
- macOS 构建暂不包含代码签名，用户首次打开需在"系统设置 > 隐私与安全性"中允许

## 问题反馈

- 提交 [Issue](https://github.com/sece1024/manhuaviewer/issues) 描述问题
- 附上操作系统、应用版本和复现步骤
