---
status: current
layer: root
canonical_for:
  - docs-index
last_verified: 2026-09-17
---

# Open Video Downloader 文档路由图

本项目是 Tauri v2 桌面应用：Vue 3 + TypeScript 前端（`src/`）+ Rust 后端（`src-tauri/`），
用 yt-dlp 完成视频、音频、字幕与元数据的批量下载。应用版本 `3.2.1`，许可证 AGPL-3.0-or-later。

## 先读这些

- `docs/README.md`：本目录的用途、加载顺序与维护规则。
- `docs/manifest.yaml`：机器路由，按任务类型列出必读文档与 token 预算。
- `docs/current/product/overview.md`：产品范围与能力边界。
- `docs/current/product/glossary.md`：领域术语（Group、Leader、Override、Tracker 等）。

## 领域（业务事实）

| 领域 | 一句话职责 | 入口 |
| --- | --- | --- |
| media-queue | 入队 → 抓取元数据 → 播放列表处理 → 队列卡片状态机 → 触发下载 | `current/domains/media-queue/index.md` |
| download-engine | 把下载项翻译成 yt-dlp 参数、执行进程、解析进度/目标路径/诊断 | `current/domains/download-engine/index.md` |
| toolchain | yt-dlp / ffmpeg 二进制清单校验、下载、安装与版本追踪 | `current/domains/toolchain/index.md` |
| settings-preferences | 全局设置与本地偏好（下载位置、最近使用、窗口几何）的持久化与副作用 | `current/domains/settings-preferences/index.md` |
| auth-secrets | 加密保险库（stronghold）+ 系统钥匙串主密钥 + Cookie/账号配置 | `current/domains/auth-secrets/index.md` |
| app-lifecycle | 启动/窗口/托盘/菜单/快捷键/关闭行为/主题/i18n/通知/应用自更新 | `current/domains/app-lifecycle/index.md` |

## 平台与共享契约

- `current/platform/frontend-runtime.md`：Vite + Vue 3 + Pinia + 路由 + i18n + 隔离模式与 CSP。
- `current/platform/backend-runtime.md`：Tauri 状态管理、插件清单、异步模型、命令注册。
- `current/platform/storage.md`：应用数据目录布局（config/preferences/vault/bin/logs）与 portable/snap/MS Store 差异。
- `current/platform/observability.md`：tracing、Sentry（前后端）、分组日志缓冲。
- `current/platform/build-test-lint.md`：npm/cargo 脚本、单测/E2E、静态检查。
- `current/platform/release-and-distribution.md`：版本号、打包目标、CI 工作流、官网与签名清单。
- `current/shared/ipc-conventions.md`：命令/事件命名、载荷序列化、错误返回约定。
- `current/shared/data-ownership.md`：配置、偏好、密钥、前端状态各自的归属与生命周期。
- `current/shared/naming.md`：模块/文件/命令/i18n key/提交信息命名规范。
- `current/shared/testing-strategy.md`：测试分层与 mock 方式。

## 约束

- `current/rules/architecture.rules.md`、`coding.rules.md`、`frontend.rules.md`、`backend.rules.md`、`security.rules.md`、`forbidden.rules.md`、`documentation.rules.md`

## 过程与历史

- `changes/`：重要/复杂改动的 brief/design/tasks/verification/worklog/reviews。
- `decisions/`：长期技术取舍（ADR）。
- `postmortems/`：事故与可复用洞察，含失败类复发计数与护栏。
- `source/`：尚未成为正式基线的原始设计草案。
- `archive/`：被取代的历史内容，默认不加载。
- `evals/`：交付前的功能/bugfix/安全/UI/发布检查清单。
