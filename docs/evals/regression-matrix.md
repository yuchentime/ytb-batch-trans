---
status: current
layer: eval
domain: evals
canonical_for:
  - regression-matrix
related:
  - docs/current/product/acceptance.md
last_verified: 2026-09-17
---

# 回归矩阵（改动 → 必测）

按改动位置查表，明确"除了新功能，还要回归什么"。每一行的右侧是最小必测集。

## 改动位置 → 必测项

| 改动位置 | 必测（自动化） | 必测（手工） |
| --- | --- | --- |
| `src/stores/media/*` | `mediaStore`、`mediaGroup`、`mediaProgress`、`playlistSelection*` 单测 | 入队 → 选择 → 下载 → 暂停/恢复 → 删除；卡片按钮可用性矩阵 |
| `src/components/media-card/**` | `playlistSelectionStep`、`settingsView`(结构) 单测 | 播放列表选择（简单 + 高级）、配置页体积显示、错误步骤 |
| `src/helpers/{formats,resolutionSelection,inputFilters,partialDownload}.ts` | 对应单测（`formats`、`resolutionSelection`、`inputFilters`、`partialDownload`、`units`） | 分辨率/编码/音轨下拉项与所选分辨率的一致性 |
| `src-tauri/src/scheduling/**` | `cargo test`（dispatcher 并发/轮转/不饿死） | 多 group 并发下载、暂停后队列清空、恢复不重复 |
| `src-tauri/src/runners/ytdlp_args/**` | `cargo test`（argv 断言） | 真实下载：视频/音频/字幕/部分下载各一次 |
| `src-tauri/src/runners/ytdlp_download.rs` | `cargo test`（runner 单测） | 取消（进程树消失）、失败诊断、日志完整 |
| `src-tauri/src/parsers/**` | `cargo test`（progress/error/single） | 失败场景的 code 映射（404/私有/限流） |
| `src-tauri/src/models/**` | `cargo test`（serde 兼容用例） | 旧配置文件/旧前端字段的兼容（升级一次旧版本数据目录） |
| `src-tauri/src/state/**` | `cargo test`（paths） | 设置保存/重启保持、重置、并发上限即时生效 |
| `src-tauri/src/commands/**` | 无（薄层） | `tauri dev` 中走一遍对应功能；确认隔离白名单 |
| `src-tauri/src/binaries/**` | `cargo test`（提取器） | 删除 bin 目录后重装、`is_locked` 生效、sha256 失败路径 |
| `src-tauri/src/stronghold/**` | `cargo test`（凭据摘要） | 首次启用、重启保持、钥匙串不可用降级 |
| `src-tauri/src/{lib,window,tray,menu,i18n}.rs` | `cargo test`（paths/window 纯函数） | 启动/单实例/托盘/关闭行为/语言切换/自启动 |
| `src-tauri/locales/**`、`src/locales/**` | `npm run lint`（i18n 规则） | 切换语言看托盘/通知/界面是否全部切换；缺 key 回退英文 |
| `src-isolation/**` | `npm run build:isolation` | 修改后的白名单在真实应用中生效（新增命令可调用、未登记命令被拒） |
| `src-tauri/tauri.conf.json`、`capabilities/**` | `npm run build` | 打包冒烟（权限/CSP 变更导致的白屏） |
| `scripts/**`（清单） | `npm run manifest:gen/sign/verify`（本地密钥） | 应用侧真实安装一次 |
| `.github/workflows/**` | 工作流 dry-run（手动触发或 PR 验证） | 发布流程走一遍（可先在 fork 演练） |

## 必测旅程（跨领域）

任何改动至少跑通以下 5 条中的相关部分：

1. **J2/J4 主线**：单视频入队 → 配置 720p → 下载完成，文件存在且可播放。
2. **J3 播放列表**：播放列表入队 → 选择 2 条 → 下载 → 两个文件、编号正确。
3. **J5 控制**：下载中暂停 → 进程消失 → 恢复 → 完成。
4. **J6 失败**：无效 URL/私有视频 → 出现可读诊断、卡片为 error、日志可查看。
5. **J1/J8 环境**：删除 bin 目录 → 启动进入安装页 → 安装完成 → 下载成功；更新提示条出现（可更新形态）。

## 高风险改动（必须额外评审）

| 改动 | 风险 |
| --- | --- |
| 诊断码字符串 | 破坏历史日志与 i18n（禁止重命名，见 `rules/forbidden.rules.md`） |
| `--progress-template` / 进度字段索引 | 进度静默失效 |
| `STRONGHOLD_KEYS` 键名 | 丢失用户凭据 |
| `media_download` 的参数结构 | 下载全部失败（前端字段与 Rust 结构错位） |
| 并发/取消逻辑 | 进程泄漏、队列卡死 |
| `paths.rs` 目录解析 | 用户数据"消失"或写到错误位置 |
| `manifest` 公钥/签名流程 | 全量用户无法安装/更新 |

## Verification

- 自动化：`npm run test:unit && npm run test:e2e && npm run build`；`cd src-tauri && cargo test`。
- 手工：按上表与 5 条旅程执行，在 change 的 `verification.md` 或 PR 描述中记录结果。
