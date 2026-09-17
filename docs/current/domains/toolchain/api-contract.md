---
status: current
layer: domain
domain: toolchain
canonical_for:
  - toolchain-api-contract
related:
  - docs/current/shared/ipc-conventions.md
  - docs/current/platform/release-and-distribution.md
last_verified: 2026-09-17
---

# toolchain 接口契约

## Scope

- 应用 ↔ 清单站点（HTTP）：`manifest.json` + `manifest.sig`。
- 应用 ↔ 二进制源（HTTP）：工具归档文件。
- 前端 ↔ 后端（Tauri）：2 个命令 + 5 个事件。

## Conventions

- 清单与签名的 URL 硬编码在 `binaries_manager.rs`；公钥以十六进制硬编码（与 CI 的 `ED25519_PUB_KEY_HEX` 对应）。
- 所有网络请求使用 `reqwest::Client`，无自定义 UA/超时/重试。
- 事件名固定 `binary_*` / `binary_update_complete`；载荷字段 camelCase（`rename_entry_to` 等清单字段除外）。

## HTTP：清单

### GET manifest.json

- **URL**：`https://jely2002.github.io/youtube-dl-gui/manifest/manifest.json`（GitHub Pages，`docs/manifest/` 目录）
- **功能职责**：声明每个工具的目标版本与各平台产物（URL、sha256、解压方式）。

```json
{
  "generatedAt": "2026-09-17T00:00:00.000Z",
  "tools": {
    "yt-dlp": {
      "version": "2026.09.05",
      "files": {
        "windows-x86_64": {
          "url": "https://github.com/yt-dlp/yt-dlp/releases/download/2026.09.05/yt-dlp.exe",
          "sha256": "…64位十六进制…"
        },
        "linux-x86_64": {
          "url": "…/yt-dlp_linux",
          "sha256": "…",
          "entry": "yt-dlp_linux"
        },
        "darwin-aarch64": {
          "url": "…/yt-dlp_macos",
          "sha256": "…"
        }
      }
    },
    "ffmpeg": {
      "version": "7.x",
      "files": {
        "windows-x86_64": {
          "url": "…ffmpeg.zip",
          "sha256": "…",
          "bundle": {
            "keep_folder": true,
            "folder_name": "ffmpeg",
            "entry": "bin/ffmpeg.exe",
            "rename_entry_to": null
          }
        }
      }
    }
  }
}
```

| 字段 | 类型 | 必填 | 说明 | 允许值/格式 | 为空语义 |
| --- | --- | --- | --- | --- | --- |
| `generatedAt` | `string` | 是 | 生成时间（ISO8601） | — | 应用不解析该字段 |
| `tools` | `Record<string, ToolInfo>` | 是 | 工具名 → 信息；工具名同时决定可执行文件名（`bin/<name>[.exe]`） | `yt-dlp`、`ffmpeg` | 空对象表示无需安装 |
| `tools.<name>.version` | `string` | 是 | 目标版本；与 `metadata.json` 比较决定是否重装 | 任意版本串 | — |
| `tools.<name>.files` | `Record<string, FileInfo>` | 是 | 平台键 → 产物信息 | 键形如 `{os}-{arch}`，`os ∈ windows/linux/darwin`，`arch ∈ x86_64/aarch64/…` | 无匹配平台时跳过该工具 |
| `files.<key>.url` | `string` | 是 | 下载地址；文件名取自 URL 最后一段 | `https://…` | — |
| `files.<key>.sha256` | `string` | 是 | 归档文件 sha256（小写十六进制） | 64 字符 | 不匹配即失败 |
| `files.<key>.entry` | `string?` | 否 | 单文件归档中要提取的条目（压缩包内路径） | 如 `yt-dlp_linux` | 归档含多条目时会给 `AmbiguousArchive` |
| `files.<key>.bundle.keep_folder` | `boolean` | 是（bundle 时） | 是否保留解压出的目录（当前始终 `true`） | — | — |
| `files.<key>.bundle.folder_name` | `string?` | 否 | 解压后顶层目录名 | — | 使用压缩包自带目录 |
| `files.<key>.bundle.entry` | `string` | 是（bundle 时） | 包内可执行文件相对路径 | POSIX 风格，无前导斜杠 | — |
| `files.<key>.bundle.rename_entry_to` | `string?` | 否 | 解压后把入口重命名为该名字 | — | 不重命名 |

平台键解析（`current_platform`）：`std::env::consts::OS`（`macos` → `darwin`）+ `-` + `ARCH`。
`select_file` 先精确匹配，再退化为"前缀匹配同一 OS"（例如 `linux-x86_64` 可被 `linux-*` 覆盖）。

### GET manifest.sig

- **格式**：base64 编码的 minisign 签名（对 `manifest.json` 的**原始字节**签名）。
- **校验**：`VerifyingKey::from_bytes(hex_decode(MANIFEST_PUB_KEY))` + `verify(manifest_bytes, sig)`；
  任一环节失败 → `check`/`ensure` 返回错误（不安装任何东西）。

## 命令

### binaries_check

- **功能职责**：返回当前平台需要安装/更新的工具名列表（空数组表示无需安装）。
- **参数**：无。
- **返回**：`CheckResult { tools: string[] }`；失败返回 `Err(string)`（网络/签名/JSON 错误原文）。

### binaries_ensure

- **功能职责**：按清单安装指定工具（省略 `tools` 表示"清单中全部适用工具"）。
- **参数**：

| 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `tools` | `string[] \| null` | 否 | 只安装这些工具；`null` 表示全部 |

- **返回**：`Ok(())` 表示全部成功（或无需安装/已被锁/已有安装在进行）；`Err(string)` 为通用错误
  （`one or more tools failed to install`），每个工具的细节通过 `binary_update_complete.failures` 给出。

## 事件

### binary_download_start

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `tool` | `string` | 工具名 |
| `version` | `string` | 目标版本（写入前端卡片标签） |

### binary_download_progress

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `tool` | `string` | 工具名 |
| `received` | `number` | 已接收字节（累计） |
| `total` | `number` | `Content-Length`；缺失时后端填 `0` |

### binary_download_complete

`{ tool }`：单个工具安装成功（已通过 hash 校验且 canonical 存在）。

### binary_download_error

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `tool` / `version` | `string` | 归属 |
| `stage` | `string` | 失败阶段（见 flow.md） |
| `error` | `string` | 原始错误文本 |

### binary_update_complete

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `successes` | `string[]` | 本次成功安装的工具 |
| `failures` | `{ tool, version, stage, error }[]` | 失败明细 |
| `error` | `string?` | 写 `metadata.json` 失败时的额外错误 |

该事件在**每轮 ensure 结束时**必定发出（即使全部成功）；前端 `binaries` store 未订阅它，安装页依靠
`ensure` 的 Promise 结果与各工具的 `error` 字段判断。

## Failure And Edge Cases

- `binary_download_progress` 的 `total` 为 `0` 时前端会算出 `NaN`（除零）；当前实现会显示 0%。若上游提供 chunked 编码需先补默认值。
- `binary_update_complete` 未被前端监听，属于"后端发了但没人用"的事件；改动前先确认是否仍需要。
- 事件没有 requestId/批次号：两轮并发 ensure 的事件无法区分（依赖 `BinariesState` 锁与前端单次调用规避）。
