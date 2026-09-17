---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-flow
related:
  - docs/current/domains/media-queue/api-contract.md
  - docs/current/domains/media-queue/frontend-behavior.md
  - docs/current/domains/media-queue/backend-behavior.md
last_verified: 2026-09-17
---

# media-queue 端到端流程

## Purpose

描述一条 URL 从进入应用到被交给下载引擎所经过的所有节点、状态变化与边界，作为改动本领域时的实现约束。

## Current Behavior

### 1. 入队渠道

| 渠道 | 入口代码 | 备注 |
| --- | --- | --- |
| 顶栏输入框 | `TheHeader.vue` → `mediaStore.addUrlBatch` | 支持多行/空格/逗号分隔，`parseUrlInputText` 去重 |
| 剪贴板监听 | `TheHeader.vue` + `useClipboard`（1000ms 轮询） | 命中未读过的合法 URL 时自动入队并 `markSeen` |
| 拖放 | `useDragDrop` + `dragDropStore.handleDrop` | 支持 `text/uri-list`、`text/plain`、`.csv`/`.txt` 文件 |
| 全局快捷键 | `shortcut_action` 事件 → `mediaStore` | `media_add`（仅入队）、`media_add_and_download`、`download_all` |
| 托盘/菜单 | 同上（后端 `ShortcutPayload`） | 快捷键文案随平台变化：macOS 为 `Ctrl+Shift+*`，其它平台为 `Alt+Shift+*` |

唯一入队函数是 `dispatchMediaInfoFetch(url, fromShortcut, skipPlaylistSelection)`：
本地生成 `id`/`groupId` → 建 group → 写初始状态 → `media_info`。

### 2. 抓取与解析（后端）

`media_info` → `FetchSender` → `GenericDispatcher` → `run_ytdlp_info_fetch`：
`yt-dlp -J --flat-playlist`（含 format/input/认证/网络参数）→ 文本按行分解为
`ERROR:` / `WARNING:` 诊断事件与 `media_fatal`（非 0 退出）→ `parse_ytdlp_info` 判定类型：

- `is_live == true` → Livestream → `media_fatal`（"Livestreams unsupported"）。
- `type == "playlist"` 或存在非空 `entries` → Playlist → `media_add`（item 带 `entries`）。
- 其它 → Single → `media_add`。

### 3. 播放列表处理

前端 `processMediaAddPayload` 依据是否带 `entries` 分叉：

- 带 entries：成为 leader，状态 `playlistSelection`（若 `skipPlaylistSelection` 则立即自动展开）。
- 不带 entries：作为普通 item 累积，`processed` 自增；等于 `total` 时 `finalizePlaylistGroup`。

`finalizePlaylistGroup` 的两条路径：

| 条件 | 行为 | 结果状态 |
| --- | --- | --- |
| `total < performance.splitPlaylistThreshold` | `splitGroup` 拆成单条 group，overrides 迁移到每个新 group | 每个新 group `configure` |
| `total >= threshold` | `consolidateGroup` 合并为一个 combined group（合并 codecs/tracks/formats、时长求和） | 原 group `configure` |

用户确认选择后 `expandPlaylistGroup`：裁剪 entries → 生成 `playlist_items` override → `media_playlist_expand` 逐条抓取。

### 4. 交给下载引擎

`downloadGroup(groupId, options)` 组装的 `DownloadItem[]` 字段：
`id`、`url`、`format`（`DownloadOptions` + 编码/音轨）、`subtitleInventory`、`overrides`、
`templateContext.values`（`playlist_index`、`playlist_id/title/uploader/uploader_id/count`、`n_entries`）。

后端 `media_download` → `ensure_group_running` → download 调度器。

### 5. 暂停/恢复/重试/删除

- 暂停：`pauseGroup` 置 `paused`/`pausedList` + `group_cancel`（后端取消 watch 并杀进程树、丢弃队列）。
- 恢复：`downloadGroup` 重新下发所有非 leader 且非 `done` 的 item。
- 重试：`dispatchMediaInfoFetch(group.url)` 重新走抓取（不是简单重下）；失败时提示 `retryError`。
- 删除：`deleteGroup` 清理状态/进度/体积/诊断/目标/选项并 `group_cancel`；`deleteGroupsByState`/`deleteAllGroups` 批量执行。

## Key Nodes

| 节点 | 输入 | 职责 | 输出/状态变化 | 边界 |
| --- | --- | --- | --- | --- |
| 入口解析（前端） | 原始文本/文件/剪贴板 | 校验 `http(s)`、去重、拆分批次 | 一批合法 URL | 非法输入与重复 URL 在此丢弃 |
| `dispatchMediaInfoFetch` | URL | 生成 id、建 group、写 `fetching` | `media_info` 调用 | 同一 URL 不同 group 互不影响 |
| fetch 调度器 | `FetchRequest` | 并发限流、按 group 轮转、编号 | 条目级任务 | 与 download 调度器共享 `maxConcurrency` 上限但独立信号量 |
| `run_ytdlp_info_fetch` | URL + 参数 | 执行 yt-dlp、解析 JSON、诊断/致命错误 | `media_add` 或 `media_fatal` | 单次调用失败只影响该 item |
| `processMediaAddPayload` | `media_add` | 区分 playlist/单条、维护 total/processed | 状态迁移 | group 必须已存在，否则抛 `Orphaned media item found` |
| `finalizePlaylistGroup` | 抓满的 playlist group | 拆分或合并 + 覆盖迁移 + 通知 | `configure` | 阈值来自 settings，运行时可变 |
| `expandPlaylistGroup` | 选择的条目 | 写 `playlist_items` override 并下发 | `fetchingList` | 选择为空时抛错，不发起调用 |
| `downloadGroup` | group + options | 过滤 leader/done、组装 items | `media_download` | 无可下载 item 时直接返回 |
| 下载调度器 | `DownloadRequest::Batch` | 并发、编号、清理计数 | 下载任务 | 全部条目结束才发 `Cleanup` 移除 group |

```mermaid
flowchart TD
  A[URL / 文件 / 剪贴板 / 快捷键] --> B{入口校验与去重}
  B -- 非法 --> Z1[丢弃 + toast]
  B -- 合法 --> C[dispatchMediaInfoFetch: 建 group, 状态 fetching]
  C --> D[media_info -> fetch 调度器 -> yt-dlp -J]
  D -- 非0退出/解析失败 --> E[media_fatal -> 状态 error]
  D -- is_live --> E
  D -- single --> F[media_add -> configure]
  D -- playlist --> G[media_add(entries) -> playlistSelection]
  G -- 跳过选择 --> H[expandPlaylistGroup]
  G -- 用户确认 --> H
  H --> I[media_playlist_expand -> 逐条 media_add]
  I --> J{processed == total}
  J -- total < 阈值 --> K[splitGroup: 单条 group]
  J -- total >= 阈值 --> L[consolidateGroup: 合并组]
  K --> M[状态 configure]
  L --> M
  M --> N[downloadGroup -> media_download]
  N --> O[download 调度器 -> download-engine]
  O -- 完成 --> P[media_complete -> done]
  O -- 失败 --> E
  M -- 暂停 --> Q[group_cancel -> paused]
  Q -- 恢复 --> N
  P -- 重试 --> C
```

## Boundaries

- 前端只负责「编排 + 展示」：并发、编号、清理、进程生命周期都在后端。
- 抓取与下载共享 `maxConcurrency` 数值但是两个独立信号量，因此同时存在抓取与下载时可能瞬时超过该值（每类各 N）。
- 队列不持久化：刷新/重启后 group 全部丢失。

## Contracts

- 命令/事件字段契约：`docs/current/domains/media-queue/api-contract.md`。
- 数据结构：`docs/current/domains/media-queue/data-model.md`。
- 编号语义：`docs/current/domains/download-engine/api-contract.md` 的 `TemplateContext` 段。

## Failure And Edge Cases

- group 不存在时收到 `media_add`/`media_fatal`（例如 HMR 期间 store 被清空）会抛错；`import.meta.hot.dispose` 会主动 `deleteAllGroups`。
- 播放列表条目全部失败时 `finalizePlaylistGroup` 仍会执行，group 进入 `configure` 并显示失败计数。
- 合并组内 leader 不参与下载（`downloadGroup` 过滤 `item.entries`），只有非 leader item 会被下发。
- `media_download` 前端异常（IPC 失败）时会构造一条本地 fatal 供诊断展示。

## Verification

见 `docs/current/domains/media-queue/verification.md`。
