---
status: draft
layer: change
domain: transcribe-translate
created_at: 2026-09-17
---

# 批量转录 + AI 翻译中文 Tasks

**当前状态**：文档已全部就绪（brief/design/tasks/verification/reviews/implementation-notes/current-doc-updates），
Design Review `passed`、AC 校准 `approved`；开发者要求**先准备文档、暂不实施** —— 本清单等待开工指令。
裁剪项 C1–C7 已写回 design.md/verification.md（见 `implementation-notes.md` 的落点索引）。

## Preconditions

- [ ] Design approved（`reviews/design-review.md` 的 `DESIGN_REVIEW_PASS` 为 passed）
- [ ] AC 已对照代码核验并协商一致（`verification.md` 的 `acceptance_review_status: approved`）
- [ ] 已知必需上下文：`docs/current/domains/media-queue/*`、`docs/current/domains/download-engine/*`、`docs/current/domains/settings-preferences/data-model.md`、`docs/current/rules/*`
- [ ] 验证目标已知：`verification.md` 的 L1/L2/L3 与 AC-01…AC-20

## 阶段与任务

### Phase A — 后端主链路（L001–L003）

1. [ ] `state/config_models.rs` 新增 `transcription`/`translation`、改造 `output`、删除下载向字段；前端 `src/tauri/types/config.ts` 默认值同步；补旧配置加载单测（AC-18）
2. [ ] stronghold 新增 `ai.apiKey`；key 读取走不实现 `Debug`/`Display` 的包装类型（AC-12）；**不新增** `translation_probe`（裁剪 C3）
3. [ ] `runners/ytdlp_args/audio_args.rs`（`-f ba/best` + `-o` + playlist 开关），删除 `format_args.rs`/`output_args.rs`/`input_filter_args.rs` 与 subtitle/sponsorblock 构造；更新 argv 单测（AC-02、AC-04）
4. [ ] `runners/ffmpeg_runner.rs`：`ffprobe` 时长、固定边界 `-ss/-to -c copy` 切块；进程封装复用 `ytdlp_process.rs` 的 kill/Job Object 语义（含 W001：whisper runner 同样复用）
5. [ ] `runners/whisper_runner.rs`：argv 构造（model/device/fp16/language/condition_on_previous_text）、stderr 进度解析、退出码与 OOM 映射（AC-03、AC-17）
6. [ ] `transcribe/chunking.rs`、`transcribe/merge.rs`、`transcribe/transcript.rs` 纯函数 + 单测（AC-05、AC-06、AC-07、AC-09）
7. [ ] `translation/blocks.rs`、`translation/validate.rs`、`translation/deepseek_client.rs` + 单测（AC-10、AC-11、AC-13）
8. [ ] `scheduling/transcribe_pipeline.rs`：视频级任务、`TranscribeLimiter(1)`/`TranslateLimiter(N)`、阶段与进度事件、取消、原子写与跳过/续跑（AC-08、AC-15、AC-16、AC-19）
9. [ ] `commands/transcription/*`：只新增 `transcription_probe` 与 `transcribe_start`（裁剪 C2/C3/C4）；删除 `media_size` 的命令/事件/白名单/mock 条目；隔离白名单与 `invoke_handler` 同步（AC-01、AC-14）
10. [ ] 批次汇总 `summary.md` + 清理策略 + 通知 `batchFinished`（前后端 locale 两份 + `src/tauri/types/app.ts` 同步）
11. [ ] **文件日志**：`tracing-appender` 滚动文件层接入 `init_tracing`（与 fmt/Sentry 共存）；事件码常量 + 单行格式化辅助 + 集中脱敏 + `logging.verbose`；事件码与 design.md 第 8 节表逐字一致（AC-21–AC-26）

### Phase B — 前端覆盖式改造（L004–L006）

11. [ ] `MediaState` 新增/删除状态；卡片 `stepMap` 换为 Fetch/AudioDownload/Transcribe/Translate/Done/Skipped/Error；删除 `MediaConfigureStep` 与 `configure` 路径
12. [ ] 新 store `transcription`（probe/用量/汇总）+ 事件监听注册（`src/tauri/listeners/*` + `plugins/tauriListeners.ts`）
13. [ ] `/setup` 页：一次 `transcription_probe` 展示 whisper/CUDA/模型缓存/ffmpeg/ffprobe/key 状态；门禁禁用首页输入（AC-01；裁剪 C2/C3：无下载模型按钮与连通性预检）
14. [ ] 设置页签：转录/翻译/输出（含术语表编辑）；删除质量/字幕/过滤页与组件
15. [ ] 详情页三 tab（英文/中文/日志）+ 打开输出目录 / 用系统程序打开 txt（裁剪 C4：无 `artifacts_read`）
16. [ ] 删除下载向 helpers/components/tests（`formats`、`resolutionSelection`、`partialDownload`、`inputFilters`、`subtitles/*`、`playlistSelection` UI 部分、`MediaDownloadOptions` 等）与对应单测
17. [ ] i18n：`transcription.*`/`translation.*`/`setup.*`/`logs.*`/`errors.transcribe.*`（前后端两份 locale）

### Phase C — 验证与收尾（L007–L009）

18. [ ] L2 端到端：fake whisper/ffmpeg 脚本 + mock DeepSeek HTTP + 真实文件系统（AC 全量覆盖矩阵，含日志文件内容/轮转/降级断言）
19. [ ] L3 手工验收：真实 GPU 转录 + 真实 DeepSeek 翻译（1 个短视频 + 1 个 >20 分钟视频）+ **日志实战排查验收**（仅凭 `.log` 定位一次故意失败，AC-22/AC-24）
20. [ ] 文档同步：新建 `docs/current/domains/transcribe-translate/*`，更新 product/media-queue/download-engine/settings-preferences/app-lifecycle/platform 与 `manifest.yaml`（见 design.md 的 Current Doc Updates）
21. [ ] 清理死代码与死测试；`cargo clippy --all-targets -D warnings` 与 `npm run lint` 无新增告警

> 编号说明：Phase A 新增第 11 项（文件日志），不重排其它序号；日志相关 AC 为 AC-21–AC-27。

## Verification Commands

```bash
# L1（局部逻辑，可在 CI 运行）
cd src-tauri && cargo test
npm run test:unit

# L2（mock 外部依赖 + 真实文件系统；默认不进 CI，需显式开关）
TRANSCRIBE_E2E=1 cargo test --test transcribe_e2e      # 计划新增测试目标
npm run test:e2e

# 全量交付前
npm run lint:fix && npm run test:unit && npm run test:e2e && npm run build
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test
```

## Completion Checklist

- [ ] Code behavior matches design（含被删除能力的移除清单）
- [ ] Tests pass（L1/L2；L3 结果记录在 worklog）
- [ ] Current docs reflect final behavior（新领域 + 受影响领域 + manifest）
- [ ] Historical or superseded docs are marked（`archive/superseded/` + `superseded_by`）
- [ ] No task-only document is routed as required runtime context
