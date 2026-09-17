---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-api-contract
related:
  - docs/current/shared/ipc-conventions.md
  - docs/current/domains/download-engine/api-contract.md
last_verified: 2026-09-17
---

# media-queue 接口契约

## Scope

- 覆盖 media-queue 拥有的 5 个 Tauri 命令与 9 个事件（含 progress/destination 事件的定义位置）。
- 调用方向：前端 `invoke()` / `listen()`；参数名使用 camelCase（Tauri v2 自动转换 Rust snake_case）。
- 通用约定见 `docs/current/shared/ipc-conventions.md`。

## Conventions

- 所有 `invoke` 参数名必须与 Rust 命令签名一致（Rust `group_id` ↔ JS `groupId`）。
- 事件名固定为 snake_case 字符串；载荷字段为 camelCase。
- 返回类型只有三种：`String`（恒为 `groupId`）、`Result<String, String>`、`()`。
- 事件通过 `app.emit` 广播，前端在 `src/tauri/listeners/*` 注册，全部为全局监听（不区分窗口）。

## 命令：入队与抓取

### media_info

- **方法**：Tauri command `invoke('media_info', ...)`
- **功能职责**：对单个 URL 执行 yt-dlp 元数据抓取（`-J --flat-playlist`），结果通过 `media_add` 事件异步回传。
- **鉴权/归属**：无鉴权；`groupId` 由前端生成，必须在调用前已存在于前端 store。

#### Request Summary

| 位置 | 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| arg | `url` | `string` | 是 | 目标 URL（调用方已校验 `http(s)`） |
| arg | `id` | `string` | 是 | 该 item 的 uuid，事件会原样带回 |
| arg | `groupId` | `string` | 是 | 队列条目 uuid |
| arg | `overrides` | `DownloadOverrides \| null` | 否 | 组级覆盖；抓取阶段只使用 `inputFilters`/`input`/`auth`/`network` |

| 字段 | 类型 | 必填 | 说明 | 允许值/格式 | 为空语义 |
| --- | --- | --- | --- | --- | --- |
| `url` | `string` | 是 | yt-dlp 目标 | `https?://…` | 空串会让 yt-dlp 报错并触发 fatal |
| `id` | `string` | 是 | item uuid | UUID v4 字符串 | 不允许为空 |
| `groupId` | `string` | 是 | group uuid | UUID v4 字符串 | 不允许为空 |
| `overrides.inputFilters.playlistItems` | `string` | 否 | yt-dlp `--playlist-items` 值 | `1:5,8,-1` | `null` 表示不限制 |
| `overrides.inputFilters.minFilesize` / `maxFilesize` | `string` | 否 | 体积过滤 | 如 `10M` | `null` 表示不限 |
| `overrides.inputFilters.date` / `datebefore` / `dateafter` | `string` | 否 | 上传日期过滤 | `YYYYMMDD` | `null` 表示不限 |
| `overrides.inputFilters.matchFilters` / `breakMatchFilters` | `string` | 否 | 标题/描述过滤 | yt-dlp 语法 | `null` 表示不限 |
| `overrides.inputFilters.playlistMode` | `"singleVideo" \| "playlist"` | 否 | 混合链接处理 | — | `null` 时回落到 `input.preferVideoInMixedLinks` |
| `overrides.input.preferVideoInMixedLinks` | `boolean` | 否 | 混合链接优先单视频 | — | `null` 时用全局设置 |
| `overrides.auth.*` | 见 auth-secrets | 否 | Cookie/浏览器 | — | `null` 时用全局设置 + 钥匙串 |
| `overrides.network.*` | 见 settings-preferences | 否 | 代理/impersonate/extractor-args | — | `null` 时用全局设置 |

#### Response

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| （成功） | `string` | 原样返回 `groupId` |
| （失败） | `Err(string)` | 仅当调度器通道关闭；业务失败通过 `media_fatal` 事件表达 |

#### 事件副作用

`media_add`（Single/Playlist）、`media_fatal`（非 0 退出、解析失败、直播）、`media_diagnostic`（stderr/stdout 中被规则命中的行）、`logging_append`（原始输出行）。

### media_playlist_expand

- **功能职责**：对用户选定的播放列表条目逐条抓取元数据，每条成功都会发一次 `media_add`。
- **鉴权/归属**：无；`groupId` 必须是已存在的 playlist group。

| 位置 | 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| arg | `groupId` | `string` | 是 | 队列条目 uuid |
| arg | `entries` | `EntryItem[]` | 是 | 已裁剪的条目清单，元素为 `{ index: number, videoUrl: string }` |
| arg | `overrides` | `DownloadOverrides \| null` | 否 | 同 `media_info`；前端会写入 `inputFilters.playlistItems` |

- **为空语义**：`entries` 为空数组时后端不产生任何任务，也不会清理计数器（前端会先抛 `No playlist entries match the selected range.`）。

### media_size

- **功能职责**：按指定格式重新抓取单条媒体，回传该格式的预估体积（`media_size` 事件）。
- **鉴权/归属**：仅对单 item group 有意义；合并组不支持按需查询体积。

| 位置 | 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| arg | `url` | `string` | 是 | 目标 URL |
| arg | `id` | `string` | 是 | item uuid |
| arg | `groupId` | `string` | 是 | group uuid |
| arg | `format` | `FormatOptions` | 是 | 用于选择格式，字段见 `data-model.md` |

- **为空语义**：`format` 三个分辨率字段（`abr`/`height`/`fps`）全空时表示"最佳格式"。

### media_download

- **功能职责**：把一批 `DownloadItem` 交给下载调度器；返回 `groupId` 便于调用方关联。
- **鉴权/归属**：无；调用方负责只下发未完成、非 leader 的 item。

| 位置 | 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| arg | `groupId` | `string` | 是 | 队列条目 uuid；后端会 `ensure_group_running` |
| arg | `items` | `DownloadItem[]` | 是 | 元素字段见下 |

| 字段（`DownloadItem`） | 类型 | 必填 | 说明 | 为空语义 |
| --- | --- | --- | --- | --- |
| `id` | `string` | 是 | item uuid | 不允许为空 |
| `url` | `string` | 是 | 目标 URL | 不允许为空 |
| `format` | `FormatOptions` | 是 | 下载格式（`trackType` 等） | 字段可空，语义为"自动/最佳" |
| `subtitleInventory` | `SubtitleInventory \| null` | 否 | 该视频真实可用的字幕语言 | `null` 时 `with_subtitle_args` 不产生字幕参数（即使全局开启） |
| `overrides` | `DownloadOverrides \| null` | 否 | 组级覆盖 | `null` 时全部使用全局设置 |
| `templateContext` | `{ values: Record<string, string> }` | 是 | 路径/文件名模板变量 | 缺失键保持占位符原样 |

#### Response

| 场景 | 返回 |
| --- | --- |
| 成功 | `groupId` |
| 通道已关闭 | 后端 `unwrap()` panic（属缺陷，不应发生） |

#### 事件副作用

`media_progress`、`media_progress_stage`、`media_destination`、`media_complete`、`media_fatal`、`media_diagnostic`、`logging_append`。

### group_cancel

- **功能职责**：取消某个 group：置取消标志、杀掉正在运行的进程树、清空该 group 在两个调度队列中尚未执行的任务、删除其日志缓冲。
- **鉴权/归属**：无。

| 位置 | 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| arg | `groupId` | `string` | 是 | 队列条目 uuid；不存在时仅登记取消状态 |

- **返回**：`()`；无事件。前端负责把状态改成 `paused*` 或删除 group。

## 事件：队列与元数据

### media_add

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `groupId` | `string` | 队列条目 uuid |
| `total` | `number` | 该批次期望的 item 总数（`media_info` 为 1，`media_playlist_expand` 为选中条目数） |
| `item` | `MediaItem` | 解析结果；播放列表解析结果带 `entries`，见 `data-model.md` |

### media_size

`MediaAddPayload` + `format: FormatOptions`：用于把某格式的体积写回前端 `media-size` store。

### media_fatal

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `groupId` | `string` | 队列条目 uuid |
| `id` | `string` | item uuid |
| `exitCode` | `number \| null` | yt-dlp 退出码；内部错误为 `null` |
| `internal` | `boolean` | `true` 表示应用内部错误（spawn 失败、解析失败、规则文件损坏、IPC 异常） |
| `message` | `string` | 用户可见消息（部分为英文原文，前端按 `code` 本地化） |
| `details` | `string \| null` | 补充细节（如 stderr 片段，超过 8KiB 会截断并标注） |
| `timestamp` | `number` | Unix 毫秒 |

### media_diagnostic

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `groupId` / `id` | `string` | 归属 |
| `level` | `"error" \| "warning"` | 由 `ERROR:` / `WARNING:` 前缀决定 |
| `code` | `string` | 规则命中码，未命中为 `unknown` |
| `component` | `string \| null` | yt-dlp 组件标签（如 `ffmpeg`） |
| `message` | `string` | 去掉前缀/组件/video id 后的消息 |
| `raw` | `string` | 原始行 |
| `timestamp` | `number` | Unix 毫秒 |

## 事件：进度

### media_progress

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` / `groupId` | `string` | 归属 |
| `category` | `"video" \| "audio" \| "subtitles" \| "metadata" \| "thumbnail" \| "other"` | 由 `trackType` + 输出行推断 |
| `percentage` | `number \| null` | 0-100；部分下载时按时间段长度归一化 |
| `speedBps` | `number \| null` | 字节/秒 |
| `etaSecs` | `number \| null` | 预计剩余秒数 |

### media_progress_stage

`{ id, groupId, stage: "initializing" | "downloading" | "merging" | "remuxing" | "reencoding" | "finalizing" }`

### media_complete

`{ id, groupId }`：yt-dlp 退出码为 0 时发出，前端把 item 置 `done`。

### media_destination

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` / `groupId` | `string` | 归属 |
| `destination` | `{ confidence: number, path: string }` | 推断的最终路径 |
| `is_merged` | `boolean` | 是否来自 `[Merger]` 行 |

- `confidence` 取值：`95` VideoConvertor、`90` VideoRemuxer、`80` Merger 行、`70` Merger/ExtractAudio 标签、`65` 已下载行、`60` download 标签、`50` 其它。

## Failure And Edge Cases

- Rust 命令返回 `Err` 时前端 `invoke` 会 reject；`media_info`/`media_size`/`media_playlist_expand` 的调用方需要 try/catch（`downloadGroup` 已有兜底）。
- `media_fatal` 可能在 group 已被删除后到达（HMR/快速删除），前端会抛 `Orphaned media item found during error handling`。
- 事件没有序号或重放机制；错过的事件不会补发。
