---
status: current
layer: domain
domain: toolchain
canonical_for:
  - toolchain-backend-behavior
related:
  - docs/current/platform/storage.md
  - docs/current/domains/toolchain/flow.md
last_verified: 2026-09-17
---

# toolchain 后端行为

## Purpose

说明二进制管理器的实现边界：目录、并发、证书/签名校验、提取器能力与错误模型。

## Current Behavior

### 模块

| 文件 | 职责 |
| --- | --- |
| `binaries/binaries_manager.rs` | 清单获取与验签、平台选择、下载与校验、安装编排、事件发送、`metadata.json` 读写、bundle hoist |
| `binaries/binaries_extractor.rs` | zip / tar.bz2 解压：单文件提取、整包提取、目录穿越防护、歧义检测 |
| `binaries/binaries_state.rs` | `BinariesState`（`AtomicBool` 独占锁）与 `CheckResult` |

### 目录与文件

| 路径 | 说明 |
| --- | --- |
| `bin_dir`（见 `platform/storage.md`） | 二进制安装目录；`<tool>`（Windows 为 `<tool>.exe`）为 canonical 路径 |
| `bin_dir/metadata.json` | `{ versions: { <tool>: <version> }, is_locked: boolean }`，缺省为空版本表 |
| `bin_dir/<archive>.tmp` | 下载中的临时文件（下载完成后重命名） |
| `bin_dir/<archive>` | 校验通过的归档；解压后删除（bundle 模式亦删除） |
| `bin_dir/<bundle 目录>/…` | bundle 模式解压出的目录，随后整体 hoist 进 `bin_dir` 并删除临时目录 |

`metadata.json` 的读写走 `serde_json` + `tokio::fs`；读取失败（不存在/损坏）会回退为默认值（空版本表），
因此"损坏的 metadata"表现为"全部重装"。

### 并发与幂等

- `BinariesState::try_start()` 用 `AtomicBool::compare_exchange` 保证同一时刻只有一个 ensure 在跑；
  重复调用直接返回 `Ok(())`（不报错、不排队）。
- 单工具安装失败不影响其它工具；`metadata.versions` 只为成功者更新。
- 安装完成后 `is_locked` 被显式写回 `false`（`metadata.json` 中该字段仅由外部/手工维护为 `true`）。

### 安全

| 环节 | 机制 |
| --- | --- |
| 清单完整性 | minisign（ed25519）签名 + 应用内置公钥（`MANIFEST_PUB_KEY`） |
| 产物完整性 | 流式 sha256（边下载边计算），与清单逐一比较 |
| 归档安全 | 解压前检查每个条目的路径：禁止绝对路径与 `..`（`PathTraversal`），拒绝非文件/目录条目（`UnsupportedEntry`） |
| 传输 | 仅 HTTPS（两个目标站点均硬编码为 `https://`） |
| 权限 | Tauri capability 中未授予前端 shell 执行权限；二进制由后端直接 spawn |

### 提取器行为（`binaries_extractor.rs`）

- 单文件模式（`entry`）：在归档里找到唯一匹配条目，写入 canonical 路径；找不到 → `EntryNotFound{wanted, available}`（列出最多 20 个条目）。
- 整包模式（`bundle`）：解压全部条目到目标目录，按 `folder_name` 决定顶层目录名，可选 `rename_entry_to`。
- 未给 `entry` 且归档含多个文件 → `AmbiguousArchive`。
- 无 entry 但只有单个文件时按该文件落盘（歧义消除）。
- 解压都在 `spawn_blocking` 中执行，避免阻塞 tokio 工作线程。

### 事件发送点

| 事件 | 位置 |
| --- | --- |
| `binary_download_start` | `install_single_tool` 开始（在选择文件之后、下载之前） |
| `binary_download_progress` | `download_and_verify` 的每个 chunk |
| `binary_download_error` | `fail_stage`（任何阶段失败） |
| `binary_download_complete` | 后置检查通过之后 |
| `binary_update_complete` | `ensure` 结束时（含写 metadata 失败的场景） |

### 命令层

`binaries_check` / `binaries_ensure` 都是薄封装：取 `State<BinariesManager>` / `State<BinariesState>`，
把 `anyhow` 错误转成 `String`。命令是 async 的，会占用一个 tokio 任务直到全部下载完成。

## Boundaries

- 不校验二进制自身的可执行性（只检查文件存在，不执行 `--version`）。
- 不管理 `bin_dir` 之外的依赖（如系统 ffmpeg）；`PATH` 前置保证优先使用本地版本。
- 不清理旧版本残留文件（例如 YYYY.MM.DD 命名的旧归档），由用户在便携/清理场景自行处理。

## Contracts

- 清单字段与事件：`api-contract.md`。
- 目录形态（portable/snap/MS Store）对 `bin_dir` 的影响：`../platform/storage.md`。

## Failure And Edge Cases

- 便携版与 MS Store 版的 `bin_dir` 是 `exe_dir/bin`（可能位于只读目录），安装会失败于 `download_verify` 之后的写盘阶段。
- `bin_dir` 不存在时 `check`/`ensure` 会先 `create_dir_all`。
- 有代理需求的用户在应用内配置的代理**不会**用于二进制下载（`reqwest::Client::new()` 不读设置），仅影响 yt-dlp 请求。
- 事件在失败路径上可能只发 `binary_download_error` 而没有 `binary_download_complete`，前端需以 `error` 字段为准。
- 若清单中某工具在当前平台无条目，`build_plan` 直接跳过，`check` 返回的列表不含它（下载时会因缺少可执行文件而 `SpawnFailed`）。

## Verification

见 `docs/current/domains/toolchain/verification.md`。
