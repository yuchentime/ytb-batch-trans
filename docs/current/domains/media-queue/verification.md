---
status: current
layer: domain
domain: media-queue
canonical_for:
  - media-queue-verification
related:
  - docs/current/product/acceptance.md
  - docs/evals/regression-matrix.md
last_verified: 2026-09-17
---

# media-queue 验证

## Purpose

给出本领域「怎样算通过」的可执行验证方式：分层测试 + 手工验收步骤 + 必须保持的既有语义。

## Current Behavior

### 自动化测试映射

| 关注点 | 测试 | 命令 |
| --- | --- | --- |
| 入队与 store 行为 | `tests/unit/mediaStore.spec.ts` | `npm run test:unit` |
| 分组算法（拆分/合并/去重） | `tests/unit/mediaGroup.spec.ts` | 同上 |
| 选项/覆盖合并 | `tests/unit/mediaOptions.spec.ts`、`networkOverrides.spec.ts`、`postprocessOverrides.spec.ts`、`subtitleOverridePreferences.spec.ts` | 同上 |
| 体积缓存与匹配 | ⚠️ 当前**无自动化用例**：`media-size` store 的匹配/聚合逻辑（`size.ts`）与 `settingsView` 无关 | 同上 + 手工：切换分辨率后体积刷新、`autoLoadSize` 关闭时点“加载”、未知体积显示 |
| 目标路径存储 | `tests/unit/mediaDestination.spec.ts` | 同上 |
| 进度聚合与完成判定 | `tests/unit/mediaProgress.spec.ts` | 同上 |
| 播放列表选择 | `tests/unit/playlistSelection.spec.ts`、`playlistSelectionStep.spec.ts`、`playlistSelectionModal.spec.ts`、`playlistNumbering.spec.ts` | 同上 |
| URL 导入 | `tests/unit/urlImport.spec.ts` | 同上 |
| 端到端 | `tests/e2e/add-url.spec.ts`、`playlist-selection.spec.ts`、`queue-actions.spec.ts`、`global-selection.spec.ts`、`persist-selection.spec.ts`、`download-progress.spec.ts` | `npm run test:e2e` |
| 调度器并发/轮转 | `src-tauri/src/scheduling/dispatcher.rs` 的 `#[tokio::test]` 三例 | `cargo test`（在 `src-tauri/`） |

E2E 通过 `installTauriMock`（`tests/utils/mocks/mediaHandlers.ts` 等）注入命令返回值与事件，
不启动真实 Rust 后端；因此 E2E 验证的是前端编排，不能替代 Rust 侧测试。

### 手工验收步骤

1. **入队**：输入 `https://www.youtube.com/watch?v=…` → 卡片先出现 `fetching`，随后出现标题/时长；粘贴两行 URL 得到两张卡片。
2. **非法输入**：输入 `hello world` 或 `ftp://x` → 不入队，无请求。
3. **导入**：拖入包含 3 个 URL 与 1 行噪声的 `.txt` → 入队 3 个，toast 提示跳过 1 个。
4. **播放列表**：入队一个播放列表 → 进入选择步骤；选择 `2:4` → 只抓 3 条；条目数 < 阈值时得到 3 张单条卡片，≥ 阈值时得到 1 张合并卡片。
5. **状态可用性**：按 `前端的按钮可用性矩阵`逐项点击，确认禁用/启用符合预期。
6. **下载与暂停**：点下载 → 卡片进入 `downloading`，进度更新；暂停 → 进程被终止（系统进程列表中 `yt-dlp` 消失）、状态 `paused`；恢复 → 从剩余条目继续，已完成条目不重下。
7. **删除**：删除 group → 卡片消失、进度/诊断/体积缓存清空、路由在详情页时回到首页（`AppLayout` 的 `requiresGroup` 守卫）。
8. **通知**：开启"始终"通知策略，重复一次入队/完成/失败，确认三类通知都能弹出。

### 必须保持的既有语义（Must-Not-Break）

- `isLeader` 语义：leader 永不参与下载，只承载 group 元数据与状态。
- 拆分阈值判定使用 `total < splitPlaylistThreshold` 拆、`>=` 合并。
- 组级 override 只保存差异项，`undefined` 表示继承；不得改成"全量快照"。
- 抓取与下载使用独立信号量，二者互不阻塞。
- `playlist_index` 使用 1 起始，`autonumber` 与 `playlist_autonumber` 不得混用同一个键。

## Boundaries

- 本文件不覆盖 yt-dlp 参数正确性（属 `download-engine/verification.md`）与设置持久化（属 `settings-preferences/verification.md`）。
- 手工验收中的"进程被终止"必须在真实应用（`npm run tauri dev`）中执行；E2E 无法验证。

## Contracts

- 验收基线：`docs/current/product/acceptance.md` 的 A1–A9。
- mock 契约：`tests/utils/tauriMock.ts` + `tests/utils/mocks/*`，新增命令/事件时必须同步扩展 mock，否则 E2E 无法覆盖。

## Failure And Edge Cases

- E2E 中若新增事件但未在 mock 中注册，`listen` 会静默等待，导致用例超时而非明确失败。
- Rust 侧测试受 `glib-2.0` 等系统库影响，环境缺失时无法运行（见 `docs/current/platform/build-test-lint.md`）。

## Verification

- 全量：`npm run lint:fix && npm run test:unit && npm run build`，`cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`（在 `src-tauri/`）。
- 回归矩阵：`docs/evals/regression-matrix.md` 中 media-queue 相关行。
