---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-verification
last_verified: 2026-09-17
version: 1.0
acceptance_review_status: approved
---

# 批量转录 + AI 翻译中文 Verification

## Test Layer Taxonomy

- **L1 — Local Logic Verification**：argv 构造、固定边界切分、合并与边界告警、机械分段、block 组装与校验、错误映射、配置兼容、**日志格式化/脱敏/轮转/降级**；全部 mock，可在 CI 运行。
- **L2 — Automated Real-World Verification**：用 **fake whisper/ffmpeg 可执行脚本** + **本地 mock DeepSeek HTTP 服务** + 真实文件系统，跑完整流水线（含跳过/续跑/取消/并发上限）；不依赖 GPU 与真实 API。
- **L3 — Human-Aided Verification**：真实 GPU whisper 转录 + 真实 DeepSeek 翻译 + 人工抽验译文保真度（口误微调是否越界、术语是否正确）。

> 原则：L1 只证明代码路径正确；L2 证明联动与产物一致性；"译文是否达标"属于 L3，不允许用 L1/L2 代替。

## Acceptance Criteria（代码级验收规则）

### AC-01 环境门禁：whisper 缺失时禁止入队

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `commands/transcription/transcription_probe.rs`、新 `scheduling/transcribe_pipeline.rs`；前端 `/setup` 与首页输入 |
| 前置条件 | `transcription_probe` 返回 `whisperFound=false` |
| 预期行为 | 首页输入与 `transcribe_start` 调用被禁用；直接调用命令返回 `Err("whisperMissing")`；不产生任何 yt-dlp 调用 |
| 禁止出现 | 缺 whisper 仍进入下载阶段；静默回退到 CPU 或其它转录实现 |
| 判定方式 | L1 单测（probe 结果为 false 时 `transcribe_start` 拒绝）+ 前端行为检查 |

### AC-02 音频下载参数只含音频能力

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `runners/ytdlp_args/audio_args.rs`（`build_audio_download_args`）；`runners/ytdlp_download.rs` 的调用点 |
| 前置条件 | 任意 `DownloadOverrides`（新 schema） |
| 预期行为 | argv 含 `-f ba/best` 与 `-o <output>`；网络/认证参数按 override 合并 |
| 禁止出现 | 出现 `--merge-output-format`、`--remux-video`、`--recode-video`、`--audio-format`、`--write-subs`、`--embed-subs`、`--sponsorblock-*`、`--download-sections`、`--min-filesize` 等下载向 flag |
| 判定方式 | L1 单测断言完整 argv（复用 `runners/ytdlp_args/tests.rs` 的断言风格） |

### AC-03 模型/设备默认值与进度流契约

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `runners/whisper_runner.rs`（argv 构造 + 段行解析）；`config_models.rs::TranscriptionSettings::default` |
| 前置条件 | 未修改默认配置；喂入固定 stdout/stderr 行序列作为 fixture |
| 预期行为 | argv 含 `--model small`、`--device cuda`、`--fp16 True`、`--task transcribe`、`--output_format json`、`--condition_on_previous_text False`；`device=cpu` 时不含 `--fp16 True`；段行解析**只读 stdout**（格式 `[00:00.000 --> 00:02.160]  text`），stderr 的 tqdm 行不得被解释为进度 |
| 禁止出现 | 默认走 CPU；默认模型为 `medium`/`large-v3`；从 stderr 解析段行 |
| 判定方式 | L1 单测（默认配置 → argv 快照 + fixture 行解析断言） |

### AC-04 语言参数语义

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `runners/whisper_runner.rs`；`transcription.language` |
| 前置条件 | `language=en` 与 `language=auto` 两种配置 |
| 预期行为 | `en` → argv 含 `--language en`；`auto` → 不传 `--language`（由 whisper 自动检测） |
| 禁止出现 | `auto` 时传空字符串参数 |
| 判定方式 | L1 单测 |

### AC-05 分块覆盖完整且可关闭

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `transcribe/chunking.rs::plan_chunks` |
| 前置条件 | `duration ∈ {600s, 3601s}`、`chunkMinutes ∈ {0, 20}` |
| 预期行为 | `0` → 单块 `[0, duration]`；`20` → `ceil(duration/1200)` 块，块间无空洞无重叠，覆盖并集 = `[0, duration]`（误差 ≤0.5s） |
| 禁止出现 | 空洞、重叠 >0s、块数超过 `ceil` 结果 |
| 判定方式 | L1 单测（边界值 + 求并集断言） |

### AC-06 固定边界切分与边界风险告警（评审裁剪 C1）

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `transcribe/chunking.rs::plan_chunks`（固定边界）；新 `transcribe/merge.rs`（边界风险判定） |
| 前置条件 | 目标边界 600s；构造两组 fixture：前块末段 `end=599.9`（距边界 0.1s）与 `end=598.0`；后块首段 `start=0.01` 与 `start=1.2` |
| 预期行为 | 切点严格取固定边界（不做静音探测、不调用 ffmpeg 分析）；边界风险判定命中时发出 `chunkBoundaryRisk`（warning），不阻断、不修改文本 |
| 禁止出现 | 为找静音而额外调用 `ffmpeg silencedetect`；边界风险导致该条 `error` |
| 判定方式 | L1 单测（切点断言 + 两组件 fixture 的告警断言） |

### AC-07 合并单调且零丢失

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `transcribe/merge.rs::merge_segments` |
| 前置条件 | 3 个 chunk 的 segments（含时间偏移） |
| 预期行为 | 输出按 `start` 单调不减；段数 = 各 chunk 段数之和；时间已平移正确（chunk2 首段 start ≈ offset） |
| 禁止出现 | 段丢失、时间回退、重复 id |
| 判定方式 | L1 单测 |

### AC-08 产物完整性决定推进（原子写）

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `transcribe/artifacts.rs`（`write_atomic` + `is_complete`） |
| 前置条件 | 写入过程被中断（模拟 `.tmp` 存在但目标缺失） |
| 预期行为 | 只有 rename 成功后 `is_complete` 为真；`.tmp` 残留不影响判定；转录未完成时不写 `transcript.en.txt` |
| 禁止出现 | 半截 JSON 被当作完成产物；txt 先于 segments.json 生成 |
| 判定方式 | L1 单测（临时目录 + 中断模拟） |

### AC-09 原稿零 LLM 改写

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `transcribe/transcript.rs::format_english_transcript`；`translation/*` 不得被其引用 |
| 前置条件 | 注入一个必定失败的翻译客户端（或 `deepseekAuthFailed`） |
| 预期行为 | `transcript.en.txt` 仍生成，且内容 == `segments` 文本按规则拼接（逐字相等，仅空白差异） |
| 禁止出现 | 原稿内容来自 LLM；原稿在翻译失败时缺失 |
| 判定方式 | L1 单测（拼接等价断言）+ 代码走查（`transcript.rs` 无 `translation::` 依赖） |

### AC-10 翻译契约校验（id/顺序/非空）

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `translation/validate.rs::validate_block_response` |
| 前置条件 | 请求 id `[1,2,3]`；响应分别为正常、缺 id、顺序错、空 `zh` |
| 预期行为 | 仅正常响应通过；其余返回 `translationContractViolation`，**不落盘** `zh.blocks.json` |
| 禁止出现 | 缺 id 时用原文占位落盘；顺序错被重排后接受 |
| 判定方式 | L1 单测（4 个用例） |

### AC-11 数字保真告警

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `translation/validate.rs::check_numeric_fidelity` |
| 前置条件 | 原文含 `2024`、`90%`；译文缺失其中一个 |
| 预期行为 | 返回缺失项；调用方发出 `translationFidelityWarning`，**不阻断** |
| 禁止出现 | 因告警判定该块失败；告警被静默丢弃 |
| 判定方式 | L1 单测 + 事件断言 |

### AC-12 密钥只从 stronghold 读取且不入日志/配置

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `translation/deepseek_client.rs`（key 读取）、`stronghold/stronghold_state.rs`（新键 `ai.apiKey`） |
| 前置条件 | 配置了测试 key（形如 `sk-test-DO-NOT-LEAK`） |
| 预期行为 | key 仅出现在 `Authorization` 头；序列化 `Config`/`Preferences` 不含该值；`tracing` 输出（debug 级）不含该值 |
| 禁止出现 | key 出现在 `config.store.json`、事件载荷、Sentry、错误消息正文 |
| 判定方式 | L1 单测（序列化断言 + 捕获日志断言）+ 代码走查（key 用不实现 `Debug`/`Display` 的包装类型） |

### AC-13 限流退避与重试上限

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `translation/deepseek_client.rs::send_with_retry` |
| 前置条件 | mock 服务先返回 429（带 `Retry-After: 1`）再返回 200；以及连续 5xx |
| 预期行为 | 429 按 `Retry-After` 退避后成功；连续失败次数 == `translation.maxRetries` 后返回 `deepseekServerError`/`deepseekRateLimited` |
| 禁止出现 | 无限重试；401 也走重试（应直接 `deepseekAuthFailed`） |
| 判定方式 | L2 测试（本地 mock HTTP，见 Required Automated Checks） |

### AC-14 输出命名、跳过与覆盖

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `transcribe/artifacts.rs::resolve_outputs`；`output.rootDir|overwrite|restrictFilenames` |
| 前置条件 | 目标目录已存在两份 txt；`overwrite=false` 与 `true` 两种配置 |
| 预期行为 | 固定文件名 `transcript.en.txt`/`transcript.zh.txt`（不可配，裁剪 C5）；`false` → 标记跳过（状态 `done` + 汇总标注），不调用 yt-dlp/whisper/DeepSeek；`true` → 重跑并覆盖（`.tmp` → rename） |
| 禁止出现 | 默认覆盖用户已有文件；跳过时仍发起网络请求；文件名来自配置字段 |
| 判定方式 | L2 测试（真实文件系统 + fake 二进制调用计数断言） |

### AC-15 清理时机

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `scheduling/transcribe_pipeline.rs`（清理步骤） |
| 前置条件 | 成功路径（双 txt 写入成功）与失败路径（翻译校验失败） |
| 预期行为 | 成功且 `keepAudio=false` → `.work/audio.*` 删除；失败 → 音频与中间产物全部保留 |
| 禁止出现 | 翻译失败仍删音频；`keepAudio=true` 时删除 |
| 判定方式 | L2 测试（文件系统断言） |

### AC-16 取消语义

| 要素 | 内容 |
| --- | --- |
| 代码位置 | `commands/group/group_cancel.rs`（复用）、新 `scheduling/transcribe_pipeline.rs` 的取消检查 |
| 前置条件 | 转录进行中调用 `group_cancel`；翻译请求在途时调用 |
| 预期行为 | 当前 yt-dlp/ffmpeg/whisper 进程被杀死；翻译在途请求完成后**不再写盘**；该条状态为 `paused`/已删除 |
| 禁止出现 | 取消后仍写 `transcript.zh.txt`；进程残留 |
| 判定方式 | L2 测试（fake 长跑二进制 + 进程存活断言）+ L3 手工确认进程树 |

### AC-17 转录并发上限为 1

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `TranscribeLimiter`（`DynamicSemaphore::new(1)`，见 `scheduling/concurrency.rs`） |
| 前置条件 | 同时入队 3 条视频 |
| 预期行为 | 任意时刻最多 1 个 whisper 进程；总顺序可观测（fake whisper 写时间戳日志） |
| 禁止出现 | 两个 whisper 并行（会导致 8GB 显存下的 OOM） |
| 判定方式 | L2 测试（时间戳区间不重叠断言） |

### AC-18 旧配置兼容

| 要素 | 内容 |
| --- | --- |
| 代码位置 | `state/json_state.rs::materialize`（现有）+ 新 `config_models.rs` |
| 前置条件 | 旧版 `config.store.json`（含 `subtitles`、`sponsorBlock`、`inputFilters`、`output.video/audio` 等已删除键） |
| 预期行为 | 加载成功；删除的键被忽略；`transcription`/`translation`/`output.rootDir` 取默认值；不 panic、不 `Err` |
| 禁止出现 | 启动失败；默认值缺失导致 `None` 解引用 |
| 判定方式 | L1 单测（把旧 JSON 快照喂给 `materialize` 断言结果） |

### AC-19 批次汇总

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `scheduling/transcribe_pipeline.rs::write_summary`；事件 `batch_summary` |
| 前置条件 | 批次含 1 成功、1 跳过、1 失败 |
| 预期行为 | `<outputRoot>/summary.md` 含每条链接状态、输出路径（成功/跳过）、失败码（失败）、token 合计；事件载荷与文件内容一致 |
| 禁止出现 | 汇总缺少失败项；token 数与累计用量不一致 |
| 判定方式 | L2 测试（文件内容 + 事件断言） |

### AC-20 新代码不含 panic 路径

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 所有新模块（`runners/whisper_runner.rs`、`runners/ffmpeg_runner.rs`、`transcribe/*`、`translation/*`、`scheduling/transcribe_pipeline.rs`、`commands/transcription/*`） |
| 前置条件 | 代码走查 |
| 预期行为 | 无 `unwrap()`/`expect()`/`panic!`（除测试）；错误一律 `Result` 传播 |
| 禁止出现 | 复刻既有 `media_download` 的 `unwrap()` 写法 |
| 判定方式 | 代码走查 + `cargo clippy`（`unwrap_used` 未启用，需人工确认；可用 grep 辅助） |

### AC-21 日志文件存在且逐行落盘

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `logging` 文件层（接入 `src-tauri/src/lib.rs::init_tracing`，与现有 fmt/Sentry 层共存） |
| 前置条件 | 应用已启动，日志目录可写；触发任一事件（如 `stage.change`） |
| 预期行为 | `<app_dir>/logs/transcribe.log` 存在且为追加模式；事件发生后 ≤1s 内文件中出现对应行（已 flush，无需关闭应用即可 `tail` 看到） |
| 禁止出现 | 事件发生后只能在前端看到、文件里没有；需要退出应用才落盘；日志层未注册却默默无文件 |
| 判定方式 | L1 单测（写入后立即读文件断言）+ L2 断言 + L3 真机 `tail -f` 观察 |

### AC-22 事件码与关联字段可检索

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `logging` 事件码常量与格式化辅助函数；流水线各阶段调用点 |
| 前置条件 | 跑完一条视频（含多个 chunk 与 block） |
| 预期行为 | 每行形如 `<ISO8601> | <LEVEL> | <event> | run=… group=… stage=… <k=v…>`；`grep 'group=<id>'` 能按时间序得到该视频从 `audio.download.start` 到 `video.done` 的完整链路（含 4 个阶段切换） |
| 禁止出现 | 多行事件、缺 `run`/`group`/`stage`、事件码与 design.md 表不一致、标题中的换行/控制字符导致断行 |
| 判定方式 | L1 单测（格式化函数对含 `\n`/超长标题的输入断言单行与截断）+ L2 链路断言 |

### AC-23 日志脱敏

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `logging` 脱敏常量/断言；`RunLogSummary`（现有）+ `deepseek_client` 请求构造 |
| 前置条件 | 配置测试 `ai.apiKey`、Cookie 文件、Bearer、自定义请求头；造一次 401 与一次校验失败 |
| 预期行为 | 文件中不出现 key/Cookie/Bearer/请求头值；yt-dlp 行只出现 `has_auth=true` 类布尔；失败响应片段截断 ≤500 字符且带 `…` 标记 |
| 禁止出现 | 密钥明文、完整 `Authorization` 头、未截断的长响应体 |
| 判定方式 | L1 单测（构造日志行后对文件做敏感值检索）+ L3 手工 `grep -i 'sk-\|cookie\|authorization'` 确认为空 |

### AC-24 错误必有日志行

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 流水线错误分支（`transcribe_pipeline.rs` + 各 runner 错误映射） |
| 前置条件 | 分别构造：whisper 退出码非 0、OOM、ffmpeg 切块失败、DeepSeek 401、契约校验失败、磁盘与日志目录不可写 |
| 预期行为 | 每个 `errorCode` 触发时至少一行 ERROR 级日志，含 `errCode`、`stage`、`exit`/`status`、以及原因摘要（截断） |
| 禁止出现 | 只发 IPC 事件/只弹通知而无文件日志；错误信息只存在于前端内存 |
| 判定方式 | L1/L2（逐个错误路径断言文件行）+ `verification.md` 错误码表逐条对照 |

### AC-25 轮转与体积上限

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `logging` 文件层轮转参数（5MB × 5） |
| 前置条件 | L1 用可注入的小阈值（如 1KB）写入超过阈值的数据；`logging.verbose=false` 与 `true` 两种情况 |
| 预期行为 | 超限后生成 `.1`、依次到 `.4`，最旧的被舍弃；磁盘总量 ≤ 单文件上限 × 5；`verbose=false` 时文件中**不含** yt-dlp/whisper/ffmpeg 的逐行原始输出（仅摘要行） |
| 禁止出现 | 无限增长的单文件；`verbose=false` 仍写入逐行输出；轮转时丢当前行 |
| 判定方式 | L1 单测（阈值注入）+ L2 体积断言 |

### AC-26 日志降级不阻断

| 要素 | 内容 |
| --- | --- |
| 代码位置 | 新 `logging` 文件层的初始化与写入失败处理 |
| 前置条件 | 把日志目录指向不可写路径（或只读目录）后跑一条视频 |
| 预期行为 | 流水线继续，两份 txt 正常生成；仅出现一次 `log.write_failed`（WARN）与一次通知；不重复刷屏 |
| 禁止出现 | 因日志不可写导致 `error`/崩溃；每行都告警一次 |
| 判定方式 | L2（只读目录 fixture）+ L3 手工 |

### AC-27 日志在 UI 可达

| 要素 | 内容 |
| --- | --- |
| 代码位置 | `transcription_probe` 返回值（`logDir`/`logFile`/`logSizeBytes`）；设置→关于页与媒体详情页 Logs tab；既有 capability `opener:allow-open-path` |
| 前置条件 | 一次探针调用成功 |
| 预期行为 | 设置页显示日志路径与当前大小并可“打开日志”；Logs tab 可“打开日志文件”；前端**不读取**日志内容 |
| 禁止出现 | 新增读取日志内容的 IPC；需要用户自己找路径 |
| 判定方式 | E2E（mock probe 返回值断言 UI 与 opener 调用）+ 手工 |

## Required Automated Checks

```bash
# L1：局部逻辑验证（CI 可跑）
cd src-tauri && cargo test
npm run test:unit

# L2：mock 外部依赖 + 真实文件系统（默认不进 CI）
TRANSCRIBE_E2E=1 cargo test --test transcribe_e2e
```

- [L1] `cargo test`：argv（AC-02/03/04/17）、chunking/merge/transcript（AC-05/06/07/08/09）、validate（AC-10/11）、config 兼容（AC-18）、**日志格式化/脱敏/轮转/降级（AC-21/22/23/24/25/26）**
- [L2] `TRANSCRIBE_E2E=1 cargo test --test transcribe_e2e`
  - 适用场景：需要"真实进程 + 真实文件系统 + HTTP 交互顺序"的联动（跳过/覆盖/清理/取消/并发/重试/汇总）
  - 前提条件：仓库内 fixture 脚本 `tests/fixtures/fake-whisper`（写时间戳日志、按 `--output_format json` 生成 `text/segments/language` 结构的 json、向 stdout 打印段行）、`fake-ffmpeg`（对 `-version`/`-ss/-to` 返回固定输出或产出空文件）、本地 mock DeepSeek HTTP（`wiremock` 或 `axum` 测试服务）
  - 预计耗时：<30s
  - 副作用：仅在临时目录写文件；无网络出口
  - 清理策略：`tempfile::TempDir` 自动清理；测试结束打印残留路径（应为空）
- [L2] `npm run test:e2e`：前端流程（入队 → 阶段/进度事件 → 跳过/失败展示 → 设置页保存/加载 → `/setup` 门禁 → **日志路径展示与“打开日志”入口 AC-27**）
  - 前提条件：`tests/utils/mocks/*` 新增 transcription/translation handlers 与新事件夹具
  - 副作用：无（纯浏览器 + mock）

## Required Manual Checks

- [L3] 真实 GPU 转录 + 真实 DeepSeek 翻译（1 个 <5 分钟视频 + 1 个 >25 分钟视频）
  - 原因：whisper/GPU/DeepSeek 无法在 CI 稳定复现（模型下载、显存、费用）
  - 执行人：开发者
  - 判断标准：两份 txt 生成；英文原稿与 whisper 直接输出逐字一致（抽查 3 段）；中文译稿无增删语义、无解释性文字、数字/术语正确；日志无 key；音频已删除
- [L3] 译文保真抽验（人工）
  - 原因：语义保真是人判断
  - 执行人：开发者
  - 判断标准：随机抽 10 段，逐段对照原文；发现"新增解释/删掉信息/概括"即判 fail（记录到 Design Deviations 或反馈升版）
- [L3] 日志实战排查验收（开发者）
  - 原因：日志的价值在于“事后能定位”，无法靠断言代替
  - 执行人：开发者
  - 判断标准：故意用错误模型名（或断网）跑一次失败，然后仅凭 `transcribe.log` 在 1 分钟内定位到出错阶段与原因（含 groupId、stage、errCode）；`grep` 关键字与 design.md 事件码一致；文件内无密钥
- [L3] 8GB 显存场景下的稳定性
  - 原因：本机实测空闲显存仅 1.23GB
  - 执行人：开发者
  - 判断标准：在占用显存的程序开启时运行，收到 `whisperOutOfMemory` 且失败可续跑（不产生半截输出）

## Must-Not-Break Checks

- [ ] 受限视频（需要登录）仍能通过已配置的 Cookie / 浏览器 Cookie / stronghold 凭据下载音频（M1）
- [ ] 代理 / impersonate / extractor-args 配置仍对 yt-dlp 生效（M2）
- [ ] 旧配置文件可正常加载，设置保存/重置/窗口几何行为不变（M3）
- [ ] yt-dlp/ffmpeg 仍由签名清单安装到 `bin_dir` 并优先使用（M4）
- [ ] API key、Cookie、Bearer、请求头不出现在日志、事件、Sentry 与配置文件中（M5）
- [ ] 托盘、关闭行为、全局快捷键、通知策略、语言切换、应用自更新行为不回归（M6）
- [ ] 现有内存分组日志（`LogStore`/`logging_append`）与 Sentry 上报行为不变，文件日志只是新增通道（M7）
- [ ] 日志文件内含可用 `group=<id>` 定位的完整链路事件，且不含密钥/Cookie/Bearer/请求头值（AC-22/AC-23）

## Regression Matrix

| Scenario | Expected Result | Layer | Check | Why It Matters |
| --- | --- | --- | --- | --- |
| whisper 未安装 | 首页禁用 + `whisperMissing` | L1/L2 | AC-01 单测 + E2E | 避免用户提交后才失败 |
| 队列 3 条同时入队 | 下载并发 ≤2、whisper 串行、翻译 ≤2 | L2 | AC-17 + 时间戳断言 | 8GB 显存下的稳定性 |
| 已存在两份 txt | 标记跳过（`done` + 汇总标注）且零网络请求 | L2 | AC-14 | 批量重跑不重复烧钱 |
| 翻译第 2 块契约失败 | 重试后失败、不写 zh txt、保留现场 | L2 | AC-10 + AC-15 | 半截译文比失败更糟 |
| 429 限流 | 退避后成功 | L2 | AC-13 | DeepSeek 限流是常态 |
| 转录中取消 | 进程被杀、无产物写入 | L2/L3 | AC-16 | 用户控制权 |
| 旧配置启动 | 正常启动、新字段默认 | L1 | AC-18 | 升级不能砖机 |
| 25 分钟视频分块 | 2 块、边界告警可观测、无文本丢失 | L1/L3 | AC-05/06/07 | 长视频的核心承诺 |
| 翻译返回解释性文字 | 校验拦截并重试/失败 | L1 | AC-10 | "语义不得增删"的第一道闸 |
| 密钥配置 | 不落盘、不进日志 | L1/L3 | AC-12 + AC-23（grep 日志文件） | 泄漏即事故 |
| whisper 失败/OOM | 错误码 + 日志 ERROR 行含 exit/tail，可定位到 chunk | L1/L2 | AC-24 | 长链路首要排查手段 |
| DeepSeek 429/5xx | 日志出现 `.retry` 行与最终 `.fail`；仍可续跑 | L2 | AC-13 + AC-24 | 限流是常态 |
| 转录中取消 | `cancel.requested` 行 + 进程被杀 | L2 | AC-16 + AC-22 | 用户控制权 |
| 日志目录只读 | 转录仍成功，仅一次 `log.write_failed` | L2 | AC-26 | 日志不能成为新的单点故障 |
| 日志超 5MB | 轮转为 `.1`，总量受限 | L1/L2 | AC-25 | 避免写满磁盘 |
| 用日志排查单条视频 | `grep 'group=…'` 得到完整链路（含阶段/块/错误） | L3 | AC-22 + L3 实战验收 | 日志的最终目的 |

## Known Guardrails

- 禁止把 L1 测试当作 L2/L3 的充分证据（尤其"翻译质量"与"GPU 行为"）。
- 禁止为了让 L2 通过而伪造 whisper/DeepSeek 的真实响应内容（fixture 必须显式标注为 fake）。
- L2 依赖 fixture 脚本与 mock 服务，不得在 CI 中真实调用 DeepSeek（费用/抖动）。
- L3 产生的成本（DeepSeek 调用）由开发者承担，验收时只跑 1–2 个视频。
- 新增错误码必须同时补前端 i18n `errors.transcribe.<code>` 与本文件表格。

## Known Anti-Patterns

| Anti-Pattern | Why It Is Wrong | Correct Approach |
| --- | --- | --- |
| 用 L1 断言"译文语义正确" | 语言质量无法由单测证明 | 只做契约/保真校验，语义交 L3 |
| 让原稿经过 LLM"顺便优化" | 违反"保持原文" | 原稿纯机械生成（AC-09） |
| 用真实 DeepSeek 跑 CI | 费用 + 不稳定 + 泄漏风险 | mock 服务（AC-13） |
| 跳过 `.tmp` 原子写 | 崩溃留下半截 txt/JSON | AC-08 |
| OOM 后自动降级 CPU | 可能静默跑几小时 | 显式失败 + 提示（AC-03） |
| 把文件日志当成唯一证据/只在文件里记错误 | 用户不看文件就无从排查 | 错误必须同时走 IPC 事件/UI（AC-24 与既有错误路径并存） |
| 为“方便”把完整请求/响应或凭证明文写进日志 | 泄漏到本地文件并可能被分享 | 集中脱敏 + 截断（AC-23） |

## When To Read History

- 改 `runners/whisper_runner.rs` 或进度解析时，读本文件的 AC-03/04/16/17。
- 改翻译 prompt/校验时，读 AC-10/11 与 design.md 的 `Alternatives Considered`。
- 改 `config_models.rs` 时，读 AC-18 与 `docs/current/domains/settings-preferences/data-model.md`。

## Acceptance Criteria Review

> 时序：Design Review 通过后、编码前执行。当前 `acceptance_review_status: pending`（Design Review 结论为 `PASS_WITH_WARNINGS`，等待开发者确认与裁剪决定后再逐条对照代码核验）。

| AC | 核验结果 | 备注（修正内容 / 协商一致结论） |
| --- | --- | --- |
| AC-01 | pass | 既有落点已核对：`src-tauri/src/lib.rs` 的 `invoke_handler`、`src-isolation/main.ts` 的 `allowedCommands`、`src/tauri/listeners/*` 注册方式；`transcription_probe`/`transcribe_start` 为本 change 新建 |
| AC-02 | revised | 原引用的 `format_args.rs`/`output_args.rs` 属本次将删除的模块；改为断言新 `audio_args.rs` 不含下载向 flag，并按 `docs/current/domains/download-engine/api-contract.md` 的实际 flag 清单校准禁用列表（新增 `--embed-thumbnail`/`--write-thumbnail`/`--sub-langs`） |
| AC-03 | revised | 新增"段行只读 **stdout**"契约（2026-09-17 真机实测：段行在 stdout、tqdm 在 stderr、json 顶层 `text/segments/language`），修正原设计"解析 stderr segment 行"的错误假设 |
| AC-04 | pass | `TranscriptionSettings.language` 与既有 argv 构造风格一致；判定为 argv 快照 |
| AC-05 | pass | `chunkMinutes=0` 与 `=20` 边界值已列为前置条件；覆盖并集断言可判定 |
| AC-06 | revised | 因裁剪 C1 由"静音对齐"改为"固定边界 + `chunkBoundaryRisk`"；判定改为"断言不调用 `silencedetect` 且告警不阻断" |
| AC-07 | pass | 与设计一致；与 AC-06 共用 fixture 断言边界告警 |
| AC-08 | pass | `write_atomic`/`is_complete` 为新模块；既有可参考实现：`binaries/binaries_manager.rs` 的 `.tmp` → rename 写入模式 |
| AC-09 | pass | 锚点为新 `transcribe/transcript.rs`；依据：原稿生成路径不引用 `translation::*`（代码走查项），无需外部服务即可判定 |
| AC-10 | pass | 契约（id 集合/顺序/非空）与 DeepSeek JSON 模式能力匹配；判定为 4 组 fixture |
| AC-11 | pass | 数字保真为纯函数；与 AC-10 同模块，判定方式明确 |
| AC-12 | pass | 既有实体核对：`stronghold/stronghold_state.rs::load_auth_secrets` 的键名前缀 `auth.*`/`video.password`；新键 `ai.apiKey` 走同一 store；`Config` 序列化断言可用 `serde_json` |
| AC-13 | pass | 既有依据：`reqwest` 已在 `src-tauri/Cargo.toml`（无新增 HTTP 库）；mock 服务已列入 L2 前置条件 |
| AC-14 | revised | 去掉 `englishFileName`/`chineseFileName`（裁剪 C5）；"`skipped` 状态"改为"标记跳过（`done` + 汇总标注）"（裁剪 C7） |
| AC-15 | pass | 清理时机为文件系统断言；`keepAudio` 为新增字段 |
| AC-16 | revised | 既有 `commands/group/group_cancel.rs` 只做 `cancel_group` + 向 fetch/download 发 `Cleanup` + 删日志；AC 已补充：新 pipeline 必须自行 `subscribe_group` 并在取消时杀 whisper/ffmpeg、且在落盘前检查取消标志 |
| AC-17 | pass | 既有实体核对：`scheduling/concurrency.rs::DynamicSemaphore::new/acquire_owned`；`TranscribeLimiter` 为本 change 新增状态 |
| AC-18 | pass | 既有实体核对：`state/json_state.rs::materialize` 的"默认值 + `json_merge`"已读源码确认 |
| AC-19 | pass | `summary.md` 字段与事件载荷需新增定义；判定（文件内容 + 事件）明确 |
| AC-20 | pass | 走查项；既有反例 `commands/media/media_download.rs` 的 `unwrap()` 已记录为"不得扩散"的债务（`docs/current/rules/architecture.rules.md`） |
| AC-21 | pass | 落点核对：`src-tauri/src/lib.rs::init_tracing` 已存在三层注册位置（fmt + Sentry），文件层可直接叠加；`PathsManager::app_dir()` 提供日志目录父路径 |
| AC-22 | pass | 事件码表已固定在 design.md 第 8 节；判定为格式化函数单测（含换行/超长标题）+ L2 链路断言 |
| AC-23 | pass | 既有可核对依据：`ytdlp_runner.rs::summarize_args_for_log` 的"只记存在性"已验证；新增部分仅需对 DeepSeek 响应片段做截断断言 |
| AC-24 | pass | 与 design.md 错误码表逐条对应；流水线已有集中错误分支点（emit 事件处），文件行在同一处产生 |
| AC-25 | pass | 轮转参数固定（5MB × 5）；L1 用小阈值注入验证轮转逻辑（不能直接写 5MB 测试数据） |
| AC-26 | pass | 降级策略为"只告警不阻断"；L2 用只读目录/不可写路径 fixture 验证 |
| AC-27 | pass | 既有能力核对：`capabilities/default.json` 已含 `opener:allow-open-path`（`**/*`），无需新权限；`transcription_probe` 为本 change 新建 |

- 评审结论：AC 已协商一致（含 5 条 revised：AC-02、AC-03、AC-06、AC-14、AC-16；其余 pass；AC-21–AC-27 为 2026-09-17 追加需求新增，均已核验落点）
- 评审方 / 确认人 / 时间：pi（AI 代理）核验 / 开发者批准（2026-09-17，方案 A：接受裁剪 C1–C7；同日追加需求：长链路日志文件）

## Version History

> 版本语义：实施前的一切迭代只算 v1.0；交付后开发者反馈才升版本（v2.0…）。实施期偏差写 design.md 的 Design Deviations。
