---
status: current
layer: product
domain: product
canonical_for:
  - product-user-journeys
related:
  - docs/current/domains/media-queue/flow.md
  - docs/current/domains/app-lifecycle/flow.md
last_verified: 2026-09-17
---

# 用户旅程

每条旅程给出：入口 → 前端行为 → 后端行为 → 用户可见结果 → 所属领域。
细节链接到对应领域文档，本文件只保证「两条路线一致」。

## J1 首次启动与运行时准备

1. 进程启动：`src-tauri/src/lib.rs` 的 `setup` 注册插件、路径、配置、偏好、窗口、托盘、调度器、强保险库。
2. 窗口以 `visible: false` 创建，前端完成 store 初始化后调用 `app_ready` 才显示（自动启动 + `autoStartMinimised` 时保持隐藏）。
3. 前端 `App.vue` 依次执行：`binaries_check` → 若缺失跳转 `/install`；`stronghold_status`；`updater_check`。
4. 安装页 `binaries_ensure` 下载 yt-dlp/ffmpeg 并显示每个工具的进度与错误。
5. 结果：进入主界面（首页为空态），或停留在安装页等待重试。

领域：app-lifecycle、toolchain。

## J2 通过输入框入队单个视频

1. 顶栏输入 URL → `useMediaStore.dispatchMediaInfoFetch`：本地生成 `itemId`/`groupId`（uuid v4）并立刻插入队列卡片（状态 `fetching`）。
2. 全局输入过滤设置被转成 `inputFilters` override 随 `media_info` 下发。
3. 后端 fetch 调度器执行 `yt-dlp -J --flat-playlist`，解析为 Single/Playlist/Livestream，通过 `media_add` 事件回传。
4. 前端 `processMediaAddPayload`：单条 item 成为 leader、group 元数据回填、通知 `videoReady`、状态置 `configure`。
5. 结果：卡片显示标题/时长/体积/分辨率选项，等待用户点击下载。

领域：media-queue。

## J3 入队播放列表并选择条目

1. 抓取结果 `type=playlist` 或存在非空 `entries` → 状态 `playlistSelection`（`skipPlaylistSelection` 时自动展开全部）。
2. 用户选择简单起止或高级多行范围，`buildPlaylistItemsSpec` 生成形如 `1:5,8,-1` 的 `playlist_items`；同时本地把 entries 裁剪为选中集合。
3. `media_playlist_expand` 逐条抓取选中条目元数据（并发受 fetch limiter 限制），每返回一条发一次 `media_add`。
4. 全部返回后 `finalizePlaylistGroup`：条目数 < `splitPlaylistThreshold` 时拆成多个单条 group，否则合并为一个 combined group；`DownloadOverrides` 随之迁移。
5. 结果：卡片进入 `configure`；用户看到「N 个条目 / 失败 X / 跳过 Y」的汇总。

领域：media-queue。

## J4 调整下载参数并开始下载

1. 卡片 `configure` 步骤内调整 trackType/分辨率/帧率/编码/音轨；底部全局选择条的值会套用到所有 group（`applyGlobal*`）。
2. 下载位置、文件名模板、字幕、网络、认证、SponsorBlock 等组级差异写入 `DownloadOverrides`（只存差异项）。
3. 点击下载：`media_download` 携带 `items[]`（每项含 `format`、`subtitleInventory`、`overrides`、`templateContext`）。
4. 后端插入 download 调度器；并发由 `DownloadLimiter` 决定，播放列表编号在入队时分配。
5. 结果：卡片进入 `downloading`，进度/阶段/目标路径通过事件流回传。

领域：media-queue、download-engine、settings-preferences。

## J5 监控进度、暂停与恢复

1. 进度事件 `media_progress` / `media_progress_stage` / `media_destination` 更新前端 store；底部总进度条与任务栏/程序坞进度同步（`src/tauri/window.ts`）。
2. 暂停调用 `group_cancel`：后端 watch 通道置 false，正在运行的 yt-dlp 进程树被杀掉，调度器丢弃该 group 的剩余条目。
3. 恢复：前端重新下发该 group 未完成的 items（`downloadGroup`），后端 `ensure_group_running` 重新登记 group。
4. 结果：`media_complete` 逐条置 `done`；全部完成时通知 `videoFinished`/`playlistFinished`。

领域：media-queue、download-engine。

## J6 失败排查与反馈

1. 非 0 退出或内部错误 → `media_fatal`：item 置 `error`，combined group 全失败时整组 `error`，并弹通知 `downloadFailed`。
2. 可识别的 stderr → `media_diagnostic`：按 `diagnostic_rules.json` 映射为稳定 `code`，前端渲染成人话（`useDiagnostic`），`code=unknown` 时允许一键上报 Sentry。
3. 日志页订阅 `logging_append`，展示该 group 的原始 yt-dlp 输出（后端环形缓冲，超限丢弃最旧数据）。
4. 结果：用户要么修正配置重试（重试会重新走 J2 的抓取流程），要么上报。

领域：download-engine、observability。

## J7 配置认证信息

1. 认证页读取 `settings.auth`（Cookie 文件/浏览器）与 stronghold 五个字段（`auth.username`/`auth.password`/`video.password`/`auth.bearer`/`auth.headers`）。
2. 保存时 Cookie 配置走 `config_set`，敏感字段走 `stronghold_set`（前端 `TextEncoder` → 字节数组）。
3. 下载时 `with_auth_args` 合并全局设置、override 与保险库密文，转成 `--cookies-from-browser` / `--cookies` / `--username` / `--password` / `--video-password` / `--add-header`。
4. 结果：受保护内容可下载；`.hold` 快照损坏或钥匙串不可用时会记录 `init_error` 并禁用凭据编辑。

领域：auth-secrets。

## J8 应用与工具更新

1. 启动与手动触发 `updater_check`：非 portable/snap/MS Store 才检查 GitHub Releases。
2. 有更新 → 提示条 → `updater_download`（进度事件）→ `updater_install` → 请求重启。
3. yt-dlp/ffmpeg 更新发生在启动时的 `binaries_check`/`binaries_ensure`，仅当清单版本或本地版本不一致时下载。
4. 结果：应用与工具链保持较新，用户无需命令行操作。

领域：app-lifecycle、toolchain。
