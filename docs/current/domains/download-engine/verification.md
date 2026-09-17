---
status: current
layer: domain
domain: download-engine
canonical_for:
  - download-engine-verification
related:
  - docs/current/product/acceptance.md
  - docs/evals/regression-matrix.md
last_verified: 2026-09-17
---

# download-engine 验证

## Purpose

给出本领域的验证手段：Rust 单测覆盖哪些契约、手工如何验证真实下载、以及必须保持的既有语义。

## Current Behavior

### 自动化覆盖

| 关注点 | 位置 | 说明 |
| --- | --- | --- |
| 参数矩阵（format/output/location/input-filters） | `src-tauri/src/runners/ytdlp_args/tests.rs` | 直接断言 argv 序列；新增/修改 flag 必须在此加用例 |
| 字幕参数与语言解析 | `ytdlp_runner.rs` 的 `mod tests` | 覆盖 embedding/auto/all/`-orig`/inventory 缺失等分支 |
| SponsorBlock 参数 | 同上 | `remove`/`mark`/`precise_cuts` 组合 |
| 日志摘要不泄漏 | 同上（`summary_*`） | 包括"值以 `-` 开头"的回归用例 |
| 模板渲染与路径消毒 | `template_context.rs` 的 `mod tests` | 含目录穿越用例 `../../malicious` |
| 覆盖合并三态 | `override_resolver.rs` 的 `mod tests` | 嵌套 patch、列表整体替换、AuthSecrets 合并 |
| 进度/阶段/目标路径 | `parsers/ytdlp_progress.rs`（同文件测试） | 行序列 → 事件序列 |
| 错误规则匹配 | `parsers/ytdlp_error.rs` | 组件过滤、跳过语义、fallback |
| 信息解析 | `parsers/ytdlp_single/tests.rs` | 格式/编码/音轨/字幕清单 |
| 前端消费 | `tests/unit/mediaProgress.spec.ts`、`mediaDestination.spec.ts`、`skippedDiagnostics.spec.ts` | 事件 → UI 状态 |

运行方式（在 `src-tauri/`）：`cargo test`；前端：`npm run test:unit`。

### 手工验收步骤（真实 yt-dlp）

前置：`npm run tauri dev`，确保已安装 yt-dlp/ffmpeg（首页不跳转 `/install`）。

1. **基础下载**：`both` + 720p → 完成后的文件分辨率/容器与选择一致；`ffprobe` 可见元数据与缩略图（默认开启）。
2. **纯音频**：`audio` + mp3 → 产出 `.mp3`，文件名使用 `audioFileNameTemplate`。
3. **字幕**：全局开启字幕 + 目标视频有多语言字幕 → 嵌入模式产出无独立字幕文件；关闭嵌入则产出同名字幕文件；请求不存在的语言 → 不报错、不下其它语言。
4. **端口/部分下载**：选择一个时间段 → `--download-sections` 生效、章节未嵌入、进度按区段时长走满。
5. **认证**：配置 Cookie 文件/浏览器 → 能抓到需要登录的内容；`--username/--password` 路径用需登录的站点验证。
6. **代理**：开启代理并填错端口 → 出现 `unableToConnectToProxy` 诊断（而不是 unknown）。
7. **取消**：下载中暂停 → 进程树（含 ffmpeg 子进程）消失；恢复 → 继续且不产生半个文件被覆盖的重复记录。
8. **失败诊断**：输入一个已被删除的视频 → `videoNotFoundOrPrivate`；输入私有视频 → `playlistPrivateOrMissing`/`videoNotFoundOrPrivate`；输入不支持站点 → `urlUnsupported`。
9. **日志**：媒体详情页 Logs 显示原始输出；长任务下缓冲不会无限增长（`log_store` 上限）。

### 必须保持的既有语义（Must-Not-Break）

- 参数顺序：URL 必须最后；`-o` 必须在 URL 之前；`--output-na-placeholder ""` 必须存在。
- 认证值绝不出现在日志/Sentry；`RunLogSummary` 只含布尔与计数。
- 退出码 `0` 是唯一成功判据；取消不得写成 fatal。
- 进度/阶段事件只在状态真正变化时发出（阶段不回退、重复阶段不重发）。
- 未知模板字段必须原样保留（交给 yt-dlp 填），已替换值必须消毒 `/ \ |` 与控制字符。
- 诊断码只能来自 `diagnostic_rules.json`（或解析器内置的 `input_filter_skipped`），不得在别处硬编码字符串匹配。

## Boundaries

- 真实下载验收依赖外网与目标站点可用性；CI 不执行（`cargo test` 只覆盖纯函数）。
- Rust 测试在缺少 `glib-2.0` 等系统库的环境会失败（见 `docs/current/platform/build-test-lint.md`）。

## Contracts

- 验收基线：`docs/current/product/acceptance.md` 的 A10–A15、A23–A25。
- i18n 契约：新增 code 必须补 `errors.runner.<code>.message` / `.shortMessage`。

## Failure And Edge Cases

- 若只改 `diagnostic_rules.json` 而未补 i18n，前端会回退显示英文原文（不报错但体验退化），属于应被 review 拦下的问题。
- 修改 `--progress-template` 时若漏改解析器索引，进度会静默变成"只有速率/ETA"或整行丢弃；必须同时跑 `cargo test` 与手工下载。

## Verification

- 全量：`cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`；前端 `npm run test:unit`。
- 回归矩阵：`docs/evals/regression-matrix.md` 中 download-engine 相关行。
