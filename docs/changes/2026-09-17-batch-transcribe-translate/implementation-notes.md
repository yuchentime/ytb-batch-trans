# Implementation Notes: batch-transcribe-translate

持久性实现细节（跨循环仍然成立的事实、落点映射、实测证据）。循环内的一次性细节写 `worklog.md`。

## 1. 真机探针结果（2026-09-17，本机）

环境：

| 项 | 值 |
| --- | --- |
| whisper | `C:\Python314\Scripts\whisper`（PyPI `openai-whisper`，`20250625`） |
| Python / torch | 3.14.3 / 2.11.0+cu128，`torch.cuda.is_available() == True` |
| GPU | NVIDIA RTX 5050，8192 MiB，sm_120，驱动 616.56（实测空闲 1.23 GiB） |
| ffmpeg / ffprobe | 均随工具链清单安装（`scripts/sources.ts` 里 `ffmpeg`、`ffprobe` 是两个独立工具） |
| 模型缓存 | 仅 `~/.cache/whisper/tiny.en.pt`（探针时因 SHA256 不匹配被重新下载） |

探针命令与结论：

```bash
# 1) 生成 32s 语音样本（Windows SAPI TTS，用于验证真实段输出）
powershell -NoProfile -Command "Add-Type -AssemblyName System.Speech; $s=New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.SetOutputToWaveFile('speech.wav'); $s.Speak('...'); $s.Dispose()"

# 2) CPU 与 GPU 各跑一次（tiny.en，json 输出）
whisper --model tiny.en --device cpu  --fp16 False --language en --task transcribe --output_format json --output_dir out_cpu speech.wav >cpu_stdout.txt  2>cpu_stderr.txt
whisper --model tiny.en --device cuda --fp16 True  --language en --task transcribe --output_format json --output_dir out_gpu speech.wav >gpu_stdout.txt 2>gpu_stderr.txt
```

| 事实 | 证据 |
| --- | --- |
| **段行在 stdout**，格式 `[00:00.000 --> 00:02.160]  text`（`]` 后两个空格） | `cpu_stdout.txt` / `gpu_stdout.txt` 均含 6 行段；stderr 无段行 |
| **tqdm 进度与警告在 stderr**；模型下载进度也是 stderr 的 tqdm | `cpu_stderr.txt` 的 `0%|…72.1M` 行；`UserWarning: Performing inference on CPU when CUDA is available` |
| 输出文件名 = **`<输入 basename>.json`**，落在 `--output_dir` | `out/sample.json`、`out_cpu/speech.json` |
| json 顶层键 = `text`、`segments`、`language` | 探针解析输出 |
| segment 键 = `id, seek, start, end, text, tokens, temperature, avg_logprob, compression_ratio, no_speech_prob` | 探针解析输出 |
| GPU 与 CPU 的 `text`/`segments` 一致；32s 音频 GPU 全流程 5s（含模型加载） | `gpu text == cpu text: True` |
| 模型缓存有 SHA256 校验，不匹配会**重新下载**（影响首次启动耗时） | 探针 stderr 的 `exists, but the SHA256 checksum does not match; re-downloading` |
| `tiny.en` 会把 `2024 and 90 percent` 识别成 `arely areful and joshi percent` | `out_cpu/*.json` 的 text；支持“默认 small、必要时 medium”的模型决策，也说明数字保真校验会真实触发 |

未验证（留给 L3 / Phase A）：`small` 模型的真实耗时与显存占用、`--verbose False` 下是否仍打印段行、CUDA OOM 的 stderr 文案。

## 2. 模块落点映射（新代码 → 归属）

| 新模块 | 归属领域 | 复用的既有实体 |
| --- | --- | --- |
| `commands/transcription/{transcription_probe,transcribe_start}.rs` | transcribe-translate | `commands/mod.rs` 重导出、`lib.rs` 的 `invoke_handler`、`src-isolation/main.ts` 白名单 |
| `scheduling/transcribe_pipeline.rs` | transcribe-translate | `scheduling/dispatcher.rs` 的 `GenericDispatcher` 模式、`scheduling/group_state.rs`、`scheduling/concurrency.rs::DynamicSemaphore` |
| `runners/whisper_runner.rs`、`runners/ffmpeg_runner.rs` | transcribe-translate | `runners/ytdlp_process.rs::{configure_command, platform_process_from_child, kill_platform_process}`（W001 强制） |
| `transcribe/{chunking,merge,transcript,artifacts}.rs` | transcribe-translate | `runners/template_context.rs` 的纯函数测试风格、`binaries_manager.rs` 的 `.tmp` → rename 模式 |
| `translation/{blocks,validate,deepseek_client}.rs` | transcribe-translate | `reqwest`（已在 `Cargo.toml`）、`stronghold/stronghold_state.rs` |
| `runners/ytdlp_args/audio_args.rs` | download-engine | 替代 `format_args.rs`/`output_args.rs`/`input_filter_args.rs`；沿用 `override_resolver.rs` 的三态合并 |
| `runners/ytdlp_args/network_args.rs`、`auth_args.rs` | download-engine | 自 `ytdlp_runner.rs` 抽出的纯函数：runner 与新音频 argv 共用一份网络/认证构造（M1/M2 不双实现）；`normalize_extractor_args` 随之下移 |
| `runners/ytdlp_process.rs`（扩展） | 进程基础设施 | 新增 `spawn_piped`/`ProcessEvent`/`PipedProcess`/`run_streaming`/`prepend_bin_dir_to_path`/`tail_excerpt`；`ytdlp_runner` 原 spawn/reader 代码下沉到这里，whisper/ffmpeg runner 复用同一封装（W001 的关闭依据） |
| `commands/media/media_size.rs`（删除） | media-queue | 需同步删除：`lib.rs` handler、白名单、`tests/utils/mocks/mediaHandlers.ts`、`src/stores/media/size.ts` |

## 3. 待实现清单中的易漏项（评审与核验产生）

- `commands/group/group_cancel.rs` 目前只 `cancel_group` + 向 fetch/download 调度器发 `Cleanup` + 删分组日志；
  新 pipeline **必须自行** 订阅 `subscribe_group` 的 watch 通道并在取消时杀掉 whisper/ffmpeg 进程（AC-16）。
- `Config::before_initialized`（`state/config.rs`）当前负责填 `output.downloadDir` 默认值；新增 `output.rootDir` 后必须在此填
  `<系统下载目录>/ovd-transcripts`，否则 `None` 会传到路径拼接处（AC-18 的延伸）。
- 通知新增 `batchFinished` 需同步四处：`commands/notifications.rs::NotificationKind`、`src/tauri/types/app.ts`、
  `src/locales/*.json`、`src-tauri/locales/*.json`（前后端 key 均为 `notifications.batchFinished.title|body`）。
- fixture 二进制需要可执行位/`.cmd` 包装（Windows 上 `Command::new("whisper")` 依赖 PATH 解析，测试里应支持显式路径覆盖，
  这也正是 `transcription.whisperPath` 字段存在的意义）。
- `src-isolation/main.ts` 的 `allowedCommands` 是 `Set`，删除 `media_size` 时**不要**误删 `media_*` 其它命令。

## 4. 设计裁剪的落点索引（C1–C7）

| 裁剪 | 已写回位置 |
| --- | --- |
| C1 固定切分 | design.md（Proposed Behavior §3、关键节点表、Backend Fetching Logic、Backend Behavior、Error Handling、Risks、Alternatives） |
| C2 模型自动下载 | design.md（US table `/setup` 行、IPC、Alternatives、Risks）、tasks.md 第 9/13 项 |
| C3 无 probe 预检 | design.md（IPC、Frontend Fetching Logic、Backend Fetching Logic、Alternatives）、tasks.md 第 2/9/13 项 |
| C4 无文稿预览 | design.md（详情页行、Frontend Fetching Logic、Alternatives）、tasks.md 第 15 项 |
| C5 固定语言与文件名 | design.md（config 段、Alternatives）、verification.md AC-14 |
| C6 纯文本术语表 | design.md（translation 段、blocks 行、Frontend Fetching Logic、Alternatives） |
| C7 无 skipped 状态 | design.md（状态流、§6、Frontend Behavior）、verification.md AC-14/回归矩阵 |

## 5. 文件日志实现要点（追加需求 2026-09-17）

### 5.1 文件与轮转

| 项 | 值 | 备注 |
| --- | --- | --- |
| 目录 | `<app_dir>/logs/` | `app_dir` 由 `PathsManager::app_dir()` 提供（portable/snap 形态自动跟随，见 `docs/current/platform/storage.md`） |
| 当前文件 | `transcribe.log` | 追加模式 |
| 轮转 | `transcribe.log.1` … `.4` | 单文件 5MB，共 5 份 ≤ 25MB；测试用可注入的小阈值 |
| 写入方式 | 自研 `SizeRotatingFile`（实现 `Write` + `MakeWriter`） | 同步、无独立线程：每行先在内存拼好，遇 `\n` 单次 `write(2)` 落盘（无 fsync）；不阻塞流水线（与设计的偏差见 design.md → Design Deviations） |
| 接入点 | `src-tauri/src/lib.rs::init_tracing` | 现有 registry 已挂 fmt 层与 Sentry 层，文件层作为第三层叠加；`Targets` 过滤规则与两者一致，另把 `ovd::tool_output` target 从 Sentry 层排除 |
| 日志目录 | `file_log::configure(app_dir)`（`PathsManager` 就绪后调用） | 使用惰性 `OnceLock<PathBuf>`：`init_tracing` 早于 `PathsManager`，配置前的少量启动事件只进 fmt/Sentry，不落文件 |
| 新增依赖 | 无 | 仅启用 `tracing-subscriber` 的 `time`/`local-time` feature（本地 ISO8601 时间戳）；`Cargo.lock` 需在工具链可用后由 cargo 刷新 |

### 5.2 行格式与关联字段

单行模板（字段顺序固定，便于 `grep`/`awk`）：

```text
<ISO8601 本地时间> | <LEVEL> | <event> | run=<runId> group=<groupId> stage=<stage> <k=v …>
```

- `stage ∈ fetching|downloadingAudio|transcribing|translating|writing`；不适用时省略该字段。
- 外部字符串（标题、URL、stderr 摘要、模型响应片段）必须：折叠 `\r`/`\n`/制表符为空格、剔除控制字符、按字段截断（标题 ≤80、响应片段 ≤500、stderr 摘要 ≤2KB）并附 `…` 标记；其余字段有 8KB 硬上限（避免单行日志炸弹，正常 URL 不会触及）。
- 事件码必须与 design.md 第 8 节的表**逐字一致**（例如 `whisper.oom` 而不是 `whisperOOM`）；新增事件码先改 design.md 表再改代码（L003 已按此新增 `tool.output`）。
- 实现落点：`logging/events.rs`（事件码常量 + 逐字断言测试）、`logging/file_log.rs`（格式层 `FileLogFormat`、字段访问器、脱敏/截断、`SizeRotatingFile`、`file_log_layer`）；敏感字段名黑名单也在 `file_log.rs` 集中一处（`apikey`/`password`/`bearer`/`cookie`/`token` 等，值替换为 `[redacted]`）。

### 5.3 脱敏清单（必须集中实现，不得散落）

| 类别 | 处理 |
| --- | --- |
| `ai.apiKey` | 永不记录；DeepSeek 请求只记 `model`/`block`/`tokens`/HTTP 状态/重试次数 |
| Cookie 文件路径/浏览器名 | 允许记录（路径不是凭据） |
| Cookie 内容、账号/密码/视频密码、Bearer、自定义请求头 | 永不记录；yt-dlp 侧只输出 `has_auth`/`has_cookies`/`has_browser_cookies` 布尔（现有 `RunLogSummary`） |
| 代理 URL 中的 `user:pass@` | 只记 `has_proxy=true`，不记值 |
| 完整 URL | **已定：记录完整 URL（含 query）**（开发者 2026-09-17 决定 A，与现有 `tracing::info!("… url={}")` 行为一致）；缓解：文件仅本机不外传 + UI/文档提示“外发前先检查”（W008 已关闭） |
| 模型失败响应片段 | 截断 ≤500 字符；不包含请求头/密钥 |

### 5.4 降级与测试注入

- 初始化失败（目录不可写）与服务运行中写入失败均只告警一次 `log.write_failed`，不重试、不阻断。
- L1 测试需要可注入：日志目录路径、单文件阈值、最大份数——实现时把这三个参数做成函数入参（生产代码用常量），否则 AC-25 无法在 CI 验证。
- E2E/手工验收：`tail -f <app_dir>/logs/transcribe.log` 应能实时看到事件；故意用错误模型名跑一次，仅凭日志定位到 `whisper.fail` 行。

## 6. 追加需求后的 AC 编号边界

- AC-01–AC-20：原始范围（含 5 条 revised）。
- AC-21–AC-27：长链路日志文件（2026-09-17 追加）。
- 后续若再追加 AC，从 AC-28 继续，**不得重编号**已确认的 AC。

## 7. 本机验证环境缺口（2026-09-17 实测）

- 本机 PATH、`~/.cargo`、scoop 与常见安装位置均无 `cargo`/`rustc`/`rustup`：
  **`cargo fmt` / `cargo clippy` / `cargo test` 当前无法执行**。
- 因此 Rust 侧验证按 worklog 契约标 `[quarantined: 缺少 Rust 工具链]`；不伪造通过，也不得把前端结果当作 Rust 已验证。
- 前端可验证部分照常执行：`npm run lint:fix`、`npm run test:unit`、`npm run test:e2e`
  （本机需先 `npx playwright install chromium`，否则 21 条用例全部因缺浏览器启动失败）、`npm run build`（含 `vue-tsc --noEmit`）。
- Rust 工具链可用后必须补跑：`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、`cargo test`；
  并复核 L001 中为“尚未接线的 API”加的 `#[allow(dead_code)]`（工具链到位、DeepSeek client 接线后应移除）。
- 无 cargo 时，关键纯逻辑可用一次性 Node 端口脚本做**语义**校验（把函数逐行照搬 + 真实文件系统 + 对抗输入；L004 用它发现并修复了 `parse_clock_component` 接受 `inf`/`NaN`/负数、`end < start` 未拒绝两个真实缺口，并把 U+2028/U+2029 纳入折叠）。
  注意：端口证明算法语义，**不能**替代 Rust 编译/运行验证；脚本已随仓库交付为 `scripts/port-checks.mjs`（`node scripts/port-checks.mjs`，当前 21/21；开发校验工具，不参与 CI，改动对应 Rust 逻辑时需同步）。
