---
status: current
layer: product
domain: product
canonical_for:
  - product-glossary
related:
  - docs/current/domains/media-queue/data-model.md
  - docs/current/domains/download-engine/api-contract.md
last_verified: 2026-09-17
---

# 领域术语表

术语以「代码实体 + 含义 + 所属层」的方式列出，跨文档引用时必须使用这里的名字。
`Rust` 表示以后端为准，`FE` 表示前端 pinia store/类型为准。

## 队列结构

| 术语 | 代码实体 | 含义 |
| --- | --- | --- |
| Group（队列条目） | `Group`（FE `src/tauri/types/group.ts`） | 用户的一次入队结果；可以是单个视频，也可以是从播放列表拆出/合并后的整组。所有队列操作（下载/暂停/删除/重试）都以 group 为单位。 |
| Item（媒体项） | `MediaItem` | group 内的一条 URL 及其元数据；播放列表拆分后每个 group 只有一个 item。 |
| Leader（主项） | `isLeader` | 一个 group 中被选中承载「group 级状态与元数据」的 item。`useMediaStateStore` 只给 leader 写状态，`getGroupState` 读 leader 的状态。 |
| Combined group / 合并组 | `Group.isCombined = true` | 播放列表条目数 ≥ `performance.splitPlaylistThreshold`（默认 50）时不拆分，整组保留一个 leader + 多个非 leader item，共同占一张卡片。 |
| Split group / 拆分组 | `Group.isCombined = false` | 播放列表条目数小于阈值时按条目拆成多个单条 group（`groupStore.splitGroup`）。 |
| Entries / 条目元数据 | `EntryItem { index, videoUrl }` | 播放列表展开得到的条目清单，用于选择与编号；`index` 从 0 开始。 |
| Playlist selection | `PlaylistSelection`（FE） | 用户在卡片上选择的条目范围（简单起止或高级多行），会同时决定实际抓取的 entries 与下发给 yt-dlp 的 `playlist_items`。 |
| Total / Processed / Errored | `Group.total` / `processed` / `errored` | 期望条目数、已返回（成功或失败）条目数、失败条目数；`processed === total` 触发 `finalizePlaylistGroup`。 |

## 下载配置

| 术语 | 代码实体 | 含义 |
| --- | --- | --- |
| Settings（设置） | `Config`（Rust）/ `Settings`（FE） | 全局、跨会话持久化的下载与系统配置，落盘 `config.store.json`。 |
| Preferences（偏好） | `Preferences` | 本地界面偏好：下载目录、目录模板、最近使用、窗口几何，落盘 `preferences.store.json`。 |
| Options（本组下载选项） | `DownloadOptions` | 单个 group 的分辨率/帧率/编码/音轨选择，只存在于前端内存。 |
| Overrides（组级覆盖） | `DownloadOverrides` | 只记录与全局设置不同的字段，后端用 `resolve_with_patch` 与全局设置合并后才生成参数。 |
| Resolve（合并解析） | `resolve_with_patch(base, patch)` | 后端统一的三态合并：`None` = 用全局值，`Some(v)` = 覆盖。所有 override 类型都实现 `ApplyPatch<T>`。 |
| FormatOptions | `FormatOptions` | 真正随 `media_download` 下发的下载参数（trackType/abr/height/fps/编码/音轨）。 |
| TrackType | `both` / `audio` / `video` | 下载内容类型；决定 selector、输出模板（视频/音频文件名模板）与进度类别。 |
| Postprocess preset | `VideoPostprocessPreset` / `AudioPostprocessPreset` | 内置 ffmpeg 后处理预设（fps30/mp42/自定义）。`TranscodePolicy` 决定是否允许重新编码。 |

## 运行期

| 术语 | 代码实体 | 含义 |
| --- | --- | --- |
| Pipeline / Dispatcher | `GenericDispatcher`、`FetchEntry`、`DownloadEntry` | 后端通用调度循环：按 group 轮转、受 `DynamicSemaphore` 并发限制、负责编号分配与清理。 |
| Fetch pipeline | `FetchRequest`（Initial/Playlist/Size/SizePlaylist） | 只取元数据/体积的任务流。 |
| Download pipeline | `DownloadRequest::Batch` | 实际下载任务流。 |
| Limiter | `DownloadLimiter` / `FetchLimiter` | 两个独立的 `DynamicSemaphore`，上限为 `performance.maxConcurrency`，设置变更时热调整。 |
| Diagnostic（诊断） | `DiagnosticEvent` / `MediaDiagnostic` | yt-dlp stderr/stdout 中被规则命中的错误或警告，带稳定 `code`（见 `diagnostic_rules.json`）。 |
| Fatal | `MediaFatalPayload` | 单条 item 的终止性失败（非 0 退出、无法 spawn、事件流中断等），会把 item 置为 `error`。 |
| Skipped（被过滤跳过） | `input_filter_skipped` | 因体积/日期/关键词过滤被 yt-dlp 跳过的条目，是 warning 而非错误。 |
| Destination | `MediaDestination` | 从 yt-dlp 输出推断出的最终文件路径，带 `confidence`（0-100）与 `is_merged`。 |
| Stage（阶段） | `ProgressStage` | `initializing / downloading / merging / remuxing / reencoding / finalizing`。 |
| Template context | `TemplateContext.values` | 文件名/目录模板渲染用的键值（`playlist_index`、`autonumber`、`playlist_title` 等）。 |

## 安全与运维

| 术语 | 代码实体 | 含义 |
| --- | --- | --- |
| Stronghold / vault | `StrongholdState`、`vault.hold` | 加密保存账号密码、视频密码、Bearer Token、请求头；主密钥存放在系统钥匙串（service `com.jelleglebbeek.youtube-dl-gui`，account `master_key`）。 |
| Isolation pattern | `src-isolation/main.ts` | Tauri 隔离模式白名单，限制前端可调用的命令集合与插件权限。 |
| Manifest（二进制清单） | `scripts/gen-manifest.ts` → `manifest.json` + `manifest.sig` | 由 CI 生成并用 minisign 私钥签名；应用用内置公钥校验后才安装 yt-dlp/ffmpeg。 |
| Environment type | `installed` / `portable` / `snap` / `microsoft-store` | `PathsManager` 探测出的运行形态，决定数据目录、bin 目录与是否允许自更新。 |
| Locked metadata | `metadata.json.is_locked` | 二进制目录被标记为「不自动更新」时，`check`/`ensure` 直接跳过。 |
