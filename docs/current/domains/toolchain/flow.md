---
status: current
layer: domain
domain: toolchain
canonical_for:
  - toolchain-flow
related:
  - docs/current/domains/toolchain/api-contract.md
  - docs/current/domains/toolchain/frontend-behavior.md
last_verified: 2026-09-17
---

# toolchain 流程

## Purpose

描述二进制从"清单"到"可执行文件"的每一步与失败点，作为修改安装逻辑或排查安装失败的依据。

## Current Behavior

```mermaid
flowchart TD
  A[启动: Rust setup 注册 BinariesState + BinariesManager] --> B[前端 App.vue checkTools]
  B -- settings.update.updateBinaries=false --> Z1[跳过]
  B --> C[binaries_check]
  C --> D{metadata.json.is_locked?}
  D -- 是 --> Z2[返回空列表, 不安装]
  D -- 否 --> E[fetch_manifest: GET manifest.json + manifest.sig]
  E -- 签名/公钥/JSON 失败 --> F[命令返回 Err -> 安装页不出现]
  E --> G[build_plan: 版本不一致或文件缺失的工具有哪些]
  G -- 空 --> Z3[直接进入主界面]
  G -- 非空 --> H[router.push /install]
  H --> I[InstallView: binaries_ensure(tools)]
  I --> J{try_start 独占锁}
  J -- 已在安装 --> Z4[直接返回]
  J --> K[逐工具 install_single_tool]
  K --> L[流式下载到 bin/<name>.tmp + sha256]
  L -- hash 不匹配 --> M[fail_stage download_verify]
  L --> N[重命名为 bin/<filename> 并解压]
  N --> O[bundle: 解压目录 -> hoist 到 bin/; 单文件: 直接落到 canonical]
  O --> P[校验 canonical 存在]
  P --> Q[metadata.versions[name] = version, emit complete]
  K --> R[全部结束后写 metadata.json + emit binary_update_complete]
  R --> S[前端: 成功 -> 5s 倒计时后进入主界面; 部分失败 -> 停留在安装页]
```

### 关键节点

| 节点 | 输入 | 职责 | 输出/状态变化 | 边界 |
| --- | --- | --- | --- | --- |
| `BinariesManager::check` | 无 | 决定"缺哪些工具" | `CheckResult{tools}` | 被 lock 或清单不可用时不同步报错给用户 |
| `fetch_manifest` | 两个 URL | minisign 签名校验（内置公钥） + JSON 解析 | `Manifest` | 签名失败即放弃，绝不安装 |
| `build_plan` | manifest + metadata + 平台 | 过滤 allow 列表、平台可用性、版本/文件是否一致 | `Vec<(name, ToolInfo)>` | 只有"版本不一致或文件缺失"才会重装 |
| `install_single_tool` | 单个工具 | 下载 → 校验 → 解压 → hoist → 后置检查 | 事件 + `ToolError` | 单工具失败不影响其它工具 |
| `download_and_verify` | url + sha256 | 流式写入 `.tmp`、边写边算 hash、进度事件 | 归档文件 | hash 不匹配时删除临时文件语义见下 |
| 提取器 | 归档文件 + entry/bundle 配置 | zip/tar.bz2 解压，防目录穿越，单文件或整包 | `bin/` 下的可执行文件 | 无 entry 且多文件 → `AmbiguousArchive` |
| `metadata.json` | 版本表 | 记录已成功安装的版本 | `{ versions, is_locked }` | 仅在**完全成功**时更新对应工具版本 |

### 版本记账与"是否重装"

`build_plan` 的重装条件（任一满足即重装）：

1. `metadata.versions[name] != manifest.tools[name].version`；
2. `bin/<canonical>` 不存在（Windows 为 `<name>.exe`）。

因此：手动删除二进制会触发重装；手动替换二进制但版本号相同不会重装。

### 解压策略

| 归档形态 | 条件 | 行为 |
| --- | --- | --- |
| 单文件 zip/bz2 | 清单给了 `entry`，无 `bundle` | 只提取该 entry 到 canonical 路径，然后删除归档 |
| 整包 zip/bz2 | 清单给了 `bundle` | 按 `folder_name`/`rename_entry_to` 解压整包，再把内容 hoist 到 `bin/`，删除归档与临时目录 |
| 压缩包内多文件且未给 `entry` | — | `AmbiguousArchive` 错误（列出前 20 个条目） |
| 含 `..` 或绝对路径的条目 | — | `PathTraversal` 错误，拒绝解压 |

### 失败阶段（`stage`）

事件与 `metadata.json` 中的失败都带阶段名，便于定位：
`select_file`、`parse_filename`、`download_verify`、`canonical_path`、`post_install_check`。
失败会发 `binary_download_error{tool,version,stage,error}`，并汇总进 `binary_update_complete.failures`。

## Boundaries

- 只有"应用更新二进制"的职责；不管理 yt-dlp 自身的 `--update`（从未调用）。
- 不做下载重试与断点续传；失败需要用户重进安装页或重启应用。
- `metadata.json` 是唯一的"已安装版本"事实来源，不读取二进制自身的 `--version`。

## Contracts

- 清单/命令/事件契约：`api-contract.md`。
- 清单生成与签名流水线：`../platform/release-and-distribution.md`。

## Failure And Edge Cases

- 应用启动时若 `binaries_check` 失败（网络不可达），`App.vue` 只 `console.error`，用户不会看到安装页——此时下载会以 `SpawnFailed` 失败并提示"Failed to spawn yt-dlp"。
- `binaries_ensure` 有进程内独占锁（`BinariesState::try_start`），并发调用会直接返回 `Ok(())`（静默忽略）。
- `is_locked = true` 时 `check`/`ensure` 都跳过（供便携/离线场景手工锁定）。
- 下载写入 `bin/<filename>.tmp`（与目标同名不同扩展）；hash 失败时该临时文件不会被清理。
- hoist 使用 `fs_extra::move_dir(content_only)`，会覆盖同名文件；若 `bin/` 中存在同名不可写文件会失败并中止该工具安装。
- bundle 模式解压后保留整个目录（如 ffmpeg 的 `bin/` 与 `lib/`），因此 `bin/` 目录里会出现额外文件，属预期。

## Verification

见 `docs/current/domains/toolchain/verification.md`。
