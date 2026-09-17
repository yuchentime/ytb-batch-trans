---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-frontend-behavior
related:
  - docs/current/domains/media-queue/backend-behavior.md
  - docs/current/domains/media-queue/data-model.md
last_verified: 2026-09-17
---

# media-queue 前端行为

## Purpose

说明队列侧前端各 store 的职责边界、组件如何驱动状态、以及可复用的派生计算，
供修改队列交互时判断"该改哪一层"。

## Current Behavior

### store 分工（`src/stores/media/`）

| store | 文件 | 职责 | 关键入口 |
| --- | --- | --- | --- |
| `media` | `media.ts` | 编排：入队、抓取派发、播放列表落地、下载/暂停/恢复/删除 | `dispatchMediaInfoFetch`、`processMediaAddPayload`、`finalizePlaylistGroup`、`expandPlaylistGroup`、`downloadGroup`、`pauseGroup`、`downloadAllGroups`、`pauseAllGroups`、`resumeAllGroups`、`deleteGroup` |
| `media-group` | `group.ts` | 队列数据结构与分组算法 | `createGroup`、`splitGroup`、`consolidateGroup`、`findGroupById/ByItemId/Leader`、`getAllFormats` |
| `media-state` | `state.ts` | item/group 状态映射 | `setState`、`getState`、`setGroupState`、`getGroupState`、`hasGroupWithState` |
| `media-options` | `options.ts` | 每组的 options/encodings/tracks/overrides + 全局副本 | `setOptions`、`setOverrides`、`applyGlobalOptions`、`applyGlobalEncodings`、`applyGlobalTracks` |
| `media-size` | `size.ts` | 每 item 的格式体积缓存 | `processMediaAddPayload`、`processMediaSizePayload`、`getSize`、`getSizeForGroup`、`requestSize` |
| `media-progress` | `progress.ts` | 进度/阶段聚合与完成判定 | `processMediaProgressPayload`、`processMediaCompletePayload`、`findGroupProgress`、`findAllProgress` |
| `media-diagnostics` | `diagnostics.ts` | 诊断与 fatal 记录、group 级错误汇总 | `processMediaDiagnosticPayload`、`processMediaFatalPayload`、`findFatalsByGroupId` |
| `media-destination` | `destination.ts` | 最终文件路径（取 confidence 最高者） | `processMediaDestinationPayload`、`findDestination` |
| `watch-clipboard` | `watchClipboard.ts` | 剪贴板监听开关与已见 URL 集合 | `enable/disable/toggle`、`hasSeen/markSeen` |
| `dragDrop` | `dragDrop.ts` | 拖放解析与批量入队 | `handleDrop` |

### 状态迁移规则（唯一权威）

| 当前 | 事件/动作 | 目标 |
| --- | --- | --- |
| — | `dispatchMediaInfoFetch` | `fetching` |
| `fetching` | `media_add`（playlist，需选择） | `playlistSelection` |
| `fetching` | `media_add`（playlist，`skipPlaylistSelection`） | `fetchingList` |
| `fetching` | `media_add`（单条） | `configure` |
| `playlistSelection` | `expandPlaylistGroup` | `fetchingList` |
| `fetching`/`fetchingList` | `media_fatal` | `error`（并 reject `waitForGroupReady`） |
| `fetchingList` | 最后一条 `media_add` → `finalizePlaylistGroup` | `configure`（拆分组：每个新 group） |
| `configure` | `downloadGroup` | `downloading` 或 `downloadingList`（`isCombined`） |
| `downloading*` | `pauseGroup` | `paused` / `pausedList` |
| `paused*` | `downloadGroup` | `downloading*` |
| `downloading*` | `media_complete`（最后一个非 leader 项） | `done` |
| `done`/`error` | `retryGroup` | 回到 `fetching`（新建 group） |

细则：

- `downloadGroup` 只给**非 `done`** 的 item 写状态，因此在合并组里可以"续下"剩余条目。
- 合并组完成判定分两种：全部 `done` → 通知 `playlistFinished` 并置组 `done`；全部"终态"（done 或 error 混合）→ 静默置组 `done`。
- 单条完成 → 通知 `videoFinished`。
- `setGroupState` 在 group 不存在时静默返回（不抛错），因此删除卡片的竞态不会崩。

### 组件与交互

| 组件 | 作用 |
| --- | --- |
| `TheHeader.vue` | URL 输入、批量解析、剪贴板监听开关、CSV/TXT 导入、输入过滤入口 |
| `HomeView.vue` | 队列渲染（`orderedGroups`）、空态、拖放覆盖层 |
| `MediaCard.vue` | 按 `stepMap` 渲染当前步骤组件 + 操作列 + 缩略图兜底 |
| `steps/PlaylistSelectionStep.vue` | 简单起止 / 高级多行选择、校验与提示 |
| `steps/PlaylistSelectionModal.vue` | 高级选择行编辑（单条/范围、增删） |
| `steps/MediaConfigureStep.vue` | 分辨率/编码/音轨选择、体积展示与按需加载、失败/跳过计数 |
| `actions/MediaCardActions.vue` | 下载/暂停/恢复/重试/删除/外部打开/偏好/元数据按钮的可用性判定 |
| `TheFooter.vue` | 全局格式选择条（写入全部 group）、总进度、队列批量操作菜单 |
| `MediaDownloadOptions.vue` | 分辨率/码率选择（`approximate` 时用 `approxVideo`/`approxAudio` 匹配最接近项） |

按钮可用性矩阵（`MediaCardActions.vue`）：

| 状态 | download | pause | resume | retry | preferences | metadata |
| --- | --- | --- | --- | --- | --- | --- |
| `fetching` / `fetchingList` / `playlistSelection` | 否 | 否 | 否 | 否 | 否 | 否 |
| `configure` | 是 | 否 | 否 | 否 | 是 | 是 |
| `downloading*` | 否 | 是 | 否 | 否 | 否 | 是 |
| `paused*` | 否 | 否 | 是 | 否 | 否 | 是 |
| `done` / `error` | 否 | 否 | 否 | 是 | 否 | 是 |

### 批量与全局操作

- `applyGlobalOptions/Encodings/Tracks`：把底部选择条的值写入所有已存在 group，并缓存为"全局值"；新 group 建立时读取全局值作为默认。
- `downloadAllGroups`：只对 `configure` 状态的 group 执行，并在完成后发一次 `queueDownloading` 通知。
- `pauseAllGroups` / `resumeAllGroups`：按 group 状态筛选；恢复时若缺少 options 会 `console.warn` 并跳过。
- `deleteAllGroups`：遍历 `groupOrder` 快照后逐个 `deleteGroup`（避免遍历中修改数组）。

### 输入过滤与全局选择

- `settingsToInputFilterOverride(settings)` 把全局输入过滤设置转成 `DownloadOverrides.inputFilters`，在入队时写入 group override（`dispatchMediaInfoFetch` 中）。
- 用户开启过滤时，顶栏按钮会高亮（`isInputFiltersActive`）。
- 播放列表选择产生的 `playlistItems` 会与该 override 合并，不清除其它过滤项。

### 队列相关通知

`notifyGroup(kind, group, params, count)` 组装 `{ title, n? }`；`fromShortcut` 的 group 会以 `force=true` 绕过"仅后台提醒"策略。
通知种类：`queueAdded`、`queueDownloading`、`queueFinished`、`videoReady`、`playlistReady`、`videoFinished`、`playlistFinished`、`downloadFailed`。

### 窗口级反馈

`src/tauri/window.ts` 的 `startWindowWatcher`（非 E2E 环境启用）：

- 任务栏/程序坞徽标 = 下载中 + 可下载数量。
- 进度条：单条下载用该条百分比，多条用 `done/total`，无数据用 `Indeterminate`。
- 从"有下载中"变为"队列全空"时，若此前并发 >1 则发 `queueFinished` 通知并 `requestUserAttention`（2 秒冷却）。

## Boundaries

- 组件不得直接 `invoke` 队列命令（除 `useGroupLog` 的日志订阅与外链打开），一律经 store。
- 组件不得直接写 `itemStates`；只能经 `media` store 的动作或 `setGroupState`。
- 派生展示值（体积、语言名、接近格式）放在 `src/helpers/`，store 内不再实现第二份。

## Contracts

- 事件监听注册集中在 `src/plugins/tauriListeners.ts`（作为 Pinia 插件安装），新增事件必须在此登记。
- `props` 约定：步骤组件统一接收 `group`（`PlaylistSelectionStep` 无 props 之外的额外输入）。

## Failure And Edge Cases

- 体积查询失败（`size === null`）显示"未知"；`undefined` 显示加载态；`autoLoadSize` 关闭时显示"加载"按钮。
- `expandPlaylistGroup` 抛错时 `PlaylistSelectionStep` 会 toast 错误原文并恢复按钮。
- `processMediaAddPayload` 对重复 `media_add`（同一 item 重复到达）会覆盖同 key 的 item，不新增。
- `useMediaStateStore.itemStates` 在删除 group 时按 item 逐个 `removeState`，避免残留。
- HMR：`main.ts` 的 `import.meta.hot.dispose` 会 `deleteAllGroups()`，防止旧 store 与新代码混用。

## Verification

见 `docs/current/domains/media-queue/verification.md`；相关单测：`tests/unit/mediaStore.spec.ts`、`mediaGroup.spec.ts`、`mediaProgress.spec.ts`、`mediaDestination.spec.ts`、`playlistSelection*.spec.ts`、`tests/e2e/queue-actions.spec.ts`、`playlist-selection.spec.ts`、`add-url.spec.ts`、`global-selection.spec.ts`。
