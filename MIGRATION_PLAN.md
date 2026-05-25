# MangaViewer: Electron → Tauri 2.0 架构迁移计划

> **版本**: 2.0  
> **日期**: 2026-05-25  
> **方案**: Tauri 2.0 + Rust 后端完全重写  
> **目标**: 减小体积 | 提升性能 | 移动端扩展

---

## 1. 执行摘要

### 1.1 迁移目标
- 🎯 **体积缩减 90%**: ~150MB (Electron) → ~15MB (Tauri)
- 🚀 **性能提升**: Rust 原生后端，系统 WebView
- 📱 **全平台支持**: macOS, Windows, Linux, iOS, Android

### 1.2 技术选型
| 组件 | 当前 (Electron) | 目标 (Tauri 2.0) |
|------|-----------------|------------------|
| 框架 | Electron 33 | Tauri 2.0 |
| 后端 | Node.js + Express | **Rust + Axum** |
| 数据库 | better-sqlite3 | **rusqlite** |
| 图片处理 | sharp | **image crate** |
| 压缩包 | 7zip-min, adm-zip, node-unrar-js | **zip, sevenz-rust, unrar** |
| 前端 | React 19 (CRA) | React 19 (复用) |
| 平台 | 桌面 | **桌面 + 移动** |

---

## 2. 现状分析

### 2.1 后端代码规模
```
总计: 2,186 行 JavaScript (16 文件)

routes/          1,301 行  (38 个路由)
services/          475 行  (扫描/归档/定时器)
db/                251 行  (SQLite 初始化 + 迁移)
utils/              39 行  (搜索解析)
config/             14 行
middleware/         11 行
index.js            73 行
```

### 2.2 核心依赖映射

| npm 包 | 用途 | Rust 替代 | 成熟度 |
|--------|------|-----------|--------|
| express | Web 框架 | **axum** | ✅ 成熟 |
| better-sqlite3 | SQLite | **rusqlite** | ✅ 成熟 |
| sharp | 图片处理 | **image** | ✅ 成熟 |
| adm-zip | ZIP 读取 | **zip** | ✅ 成熟 |
| 7zip-min | 7Z 读取 | **sevenz-rust** | ⚠️ 可用 |
| node-unrar-js | RAR 读取 | **unrar** | ⚠️ 需测试 |
| winston | 日志 | **tracing** | ✅ 成熟 |

---

## 3. 迁移阶段

### 阶段 1: 环境搭建 (1 周)

#### 3.1.1 前置条件
```bash
# 确认环境
rustc --version     # >= 1.70
cargo --version
node --version      # >= 18
pnpm --version
```

#### 3.1.2 创建 Tauri 项目结构
```bash
# 在项目根目录初始化
pnpm create tauri-app src-tauri --template vanilla

# 或手动创建目录结构
mkdir -p src-tauri/{src,icons,capabilities}
```

#### 3.1.3 目标目录结构
```
manhuaviewer/
├── src-tauri/              # Tauri + Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs         # 入口
│   │   ├── lib.rs          # Tauri 命令
│   │   ├── db/             # rusqlite 封装
│   │   │   ├── mod.rs
│   │   │   ├── schema.rs   # 表结构
│   │   │   └── migrations.rs
│   │   ├── routes/         # Axum 路由
│   │   │   ├── mod.rs
│   │   │   ├── archives.rs
│   │   │   ├── tags.rs
│   │   │   ├── categories.rs
│   │   │   ├── history.rs
│   │   │   ├── settings.rs
│   │   │   └── opds.rs
│   │   ├── services/       # 业务逻辑
│   │   │   ├── mod.rs
│   │   │   ├── scanner.rs
│   │   │   ├── archive.rs
│   │   │   └── thumbnail.rs
│   │   ├── models/         # 数据模型
│   │   │   ├── mod.rs
│   │   │   ├── archive.rs
│   │   │   ├── tag.rs
│   │   │   └── ...
│   │   └── utils/
│   │       ├── mod.rs
│   │       └── search.rs
│   └── capabilities/
│       └── default.json
├── frontend/               # 保持不变
│   └── ...
└── package.json            # 更新脚本
```

---

### 阶段 2: Rust 后端核心 (3-4 周)

#### 3.2.1 Cargo.toml 配置
```toml
[package]
name = "manhuaviewer"
version = "3.0.0"
edition = "2021"

[dependencies]
# Web 框架
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "fs"] }

# 数据库
rusqlite = { version = "0.31", features = ["bundled"] }

# 图片处理
image = "0.24"

# 压缩包
zip = "0.6"
sevenz-rust = "0.5"
# unrar = "0.5"  # 待测试

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 异步运行时
tokio = { version = "1", features = ["full"] }

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 其他
dotenvy = "0.15"
quick-xml = "0.31"
walkdir = "2"
mime_guess = "2"

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

#### 3.2.2 数据库层 (db/mod.rs)
```rust
use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(())
    }
}
```

#### 3.2.3 路由层 (routes/archives.rs)
```rust
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ArchiveQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

pub async fn list_archives(
    State(db): State<Arc<Database>>,
    Query(query): Query<ArchiveQuery>,
) -> Json<serde_json::Value> {
    // 实现搜索、过滤、分页逻辑
    todo!()
}
```

---

### 阶段 3: 压缩包处理 (1-2 周)

#### 3.3.1 统一归档接口
```rust
pub trait ArchiveReader {
    fn list_pages(&self) -> Result<Vec<String>>;
    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>>;
}

pub struct ZipArchive { /* ... */ }
pub struct SevenZArchive { /* ... */ }
pub struct RarArchive { /* ... */ }

impl ArchiveReader for ZipArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        // zip crate 实现
    }
}
```

#### 3.3.2 RAR 处理备选方案
```rust
// 方案 A: 使用 unrar crate
use unrar::Archive;

// 方案 B: FFI 绑定 libunrar (更稳定)
// 方案 C: 调用系统 unrar 命令 (降级方案)
```

---

### 阶段 4: 图片处理 (1 周)

#### 3.4.1 缩略图生成
```rust
use image::{io::Reader as ImageReader, ImageOutputFormat};

pub fn generate_thumbnail(
    input: &[u8],
    width: u32,
    quality: u8,
) -> Result<Vec<u8>> {
    let img = ImageReader::new(std::io::Cursor::new(input))
        .with_guessed_format()?
        .decode()?;
    
    let thumbnail = img.resize(width, width, image::imageops::FilterType::Lanczos3);
    
    let mut output = Vec::new();
    thumbnail.write_to(
        &mut std::io::Cursor::new(&mut output),
        ImageOutputFormat::Jpeg(quality),
    )?;
    
    Ok(output)
}
```

---

### 阶段 5: OPDS 服务器 (1 周)

#### 3.5.1 OPDS XML 生成
```rust
use quick_xml::Writer;
use quick_xml::events::{Event, BytesStart, BytesText};

pub fn generate_opds_catalog(archives: &[Archive]) -> String {
    let mut writer = Writer::new(Vec::new());
    // 生成 OPDS XML
    String::from_utf8(writer.into_inner()).unwrap()
}
```

---

### 阶段 6: 前端适配 (1-2 周)

#### 3.6.1 修改 API 调用
```javascript
// frontend/src/utils/api.js
const BASE = '/api';  // Tauri 内嵌 Axum 保持相同路径

// 无需大改！Tauri 可以 serve 静态文件并代理 API
```

#### 3.6.2 Tauri 特定功能
```javascript
// 文件选择器 (可选增强)
if (window.__TAURI__) {
  const { open } = window.__TAURI__.dialog;
  // 使用原生对话框
}
```

---

### 阶段 7: 打包发布 (1 周)

#### 3.7.1 tauri.conf.json
```json
{
  "productName": "MangaViewer",
  "version": "3.0.0",
  "identifier": "com.sece1024.manhuaviewer",
  "build": {
    "frontendDist": "../frontend/build",
    "devUrl": "http://localhost:3000",
    "beforeDevCommand": "pnpm --filter manhuaviewer-frontend start",
    "beforeBuildCommand": "pnpm --filter manhuaviewer-frontend build"
  },
  "app": {
    "windows": [{
      "title": "MangaViewer",
      "width": 1200,
      "height": 800,
      "minWidth": 800,
      "minHeight": 600,
      "titleBarStyle": "Overlay"
    }],
    "security": {
      "csp": "default-src 'self'; connect-src 'self' http://localhost:*; img-src 'self' data: blob:;"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["all"],
    "icon": ["icons/*"]
  }
}
```

#### 3.7.2 构建命令
```bash
# 开发
pnpm tauri dev

# 构建 (当前平台)
pnpm tauri build

# 构建 iOS (需要 macOS + Xcode)
pnpm tauri ios build

# 构建 Android
pnpm tauri android build
```

---

## 4. 任务分解

| 序号 | 任务 | 预计时间 | 依赖 |
|------|------|----------|------|
| 1 | Tauri 项目初始化 | 2 天 | - |
| 2 | 数据库层 (rusqlite) | 3 天 | 1 |
| 3 | Archive 模型 + CRUD | 3 天 | 2 |
| 4 | Tag/Category 模型 | 2 天 | 2 |
| 5 | History + Settings | 2 天 | 2 |
| 6 | 扫描服务 | 4 天 | 3 |
| 7 | ZIP/CBZ 支持 | 3 天 | - |
| 8 | 7Z 支持 | 2 天 | 7 |
| 9 | RAR 支持 | 3 天 | 7 |
| 10 | 缩略图生成 | 3 天 | - |
| 11 | OPDS 服务器 | 3 天 | 3 |
| 12 | 前端适配 | 5 天 | 3-11 |
| 13 | 桌面端测试 | 3 天 | 12 |
| 14 | 移动端适配 | 5 天 | 13 |
| 15 | 打包发布 | 3 天 | 14 |
| **总计** | | **~46 天** | |

---

## 5. 风险与缓解

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| RAR 支持不稳定 | 中 | 保留降级方案：调用系统 unrar 命令 |
| WebView 兼容性 | 低 | 充分测试，使用 polyfill |
| 数据库迁移 | 中 | 保留 v1→v2 迁移逻辑，添加 v2→v3 |
| 移动端文件访问 | 高 | 使用 Tauri FS 插件 + 权限配置 |
| 学习曲线 | 低 | 已有 Rust 经验 |

---

## 6. 测试清单

### 桌面端
- [ ] 应用启动/关闭
- [ ] 目录扫描
- [ ] ZIP/CBZ 阅读
- [ ] 7Z 阅读
- [ ] RAR/CBR 阅读
- [ ] 缩略图生成
- [ ] 标签管理
- [ ] 分类管理
- [ ] 阅读历史
- [ ] 设置保存
- [ ] OPDS 访问
- [ ] 备份/恢复

### 移动端
- [ ] 基本阅读流程
- [ ] 触摸交互
- [ ] 文件导入
- [ ] 横竖屏切换

---

## 7. 后续优化

1. **增量 Rust 化**: 将前端构建工具迁移到 Rust (Vite → SWC)
2. **原生渲染**: 考虑用 Rust 直接渲染漫画页面 (更高性能)
3. **云同步**: 添加 WebDAV/云存储支持
4. **AI 功能**: 标签自动识别、翻译

---

## 附录 A: Rust 依赖版本

```toml
# 截至 2026-05 的推荐版本
axum = "0.7"
rusqlite = { version = "0.31", features = ["bundled"] }
image = "0.24"
zip = "0.6"
sevenz-rust = "0.5"
tracing = "0.1"
tokio = "1"
serde = "1"
```

---

## 实施进度

### 已完成 (2026-05-25)

- [x] **阶段 1: 环境搭建** - Tauri 项目初始化
  - 创建 `src-tauri/` 目录结构
  - 配置 `Cargo.toml` 和 `tauri.conf.json`
  - 实现 Rust 后端骨架
  - 编译成功

- [x] **阶段 2: Rust 后端核心** - 数据库层和路由
  - 实现 `Database` 结构体和查询方法
  - 实现所有 API 路由处理器
  - 添加 ArchiveRow, TagRow, CategoryRow, HistoryRow 类型
  - 实现备份/恢复功能

- [x] **阶段 3: 压缩包处理** - 多格式支持
  - ZIP/CBZ 支持 (zip crate)
  - RAR/CBR 支持 (系统 unrar 命令)
  - 7Z 支持 (系统 7z 命令)

- [x] **阶段 4: 图片处理** - 缩略图生成
  - 实现 ThumbnailGenerator
  - 添加缩略图缓存
  - 支持多种图片格式

- [x] **阶段 5: OPDS 服务器** - 第三方阅读器支持
  - 实现所有 OPDS 端点
  - 支持标签和分类浏览

- [x] **阶段 6: 前端适配** - API 兼容性修复
  - 修复图片端点返回原始二进制
  - 添加正确的 HTTP 状态码
  - 修复请求体格式不匹配
  - 修复查询参数名称

### 待完成

- [ ] **阶段 7: 打包发布**
  - 创建应用图标
  - 配置构建选项
  - 测试打包流程

---

**文档状态**: 实施中  
**最后更新**: 2026-05-25
