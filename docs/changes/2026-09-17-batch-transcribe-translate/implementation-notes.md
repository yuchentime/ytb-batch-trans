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

## 7. 本机 Rust 工具链与真实门禁（2026-09-17，L006 已补装）

### 7.1 已安装的工具链

| 项 | 值 |
| --- | --- |
| rustup | 1.29.1；默认 stable **1.98.1**（host `x86_64-pc-windows-msvc`） |
| CI 固定版 | **1.94.1**（含 `clippy`、`rustfmt`、`llvm-tools-preview`），与 `rust-ci.yml` 一致 |
| MSVC / SDK | VS 2022 Build Tools VCTools（MSVC **14.44.35207** + Windows SDK **10.0.26100.0**） |
| 验证命令 | `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` |

### 7.2 在 Windows 上跑门禁的正确方式（重要）

rustc 需要 MSVC 的 `link.exe`，且直接跑会遇到两个本机特有的坑：

1. Git 的 `/usr/bin/link.exe` 与 MSVC 链接器重名；
2. PATH 里 JDK 的 `api-ms-win-*.dll` 兼容 shim（如 `api-ms-win-core-synch-l1-2-0.dll`）会遮蔽系统 DLL，
   导致链接后的进程启动即 `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)`。

因此用一层包装脚本（本次放在 `%TEMP%\msvc.cmd`，不入库）：清成最小 PATH（去掉 JDK/其它工具目录，仅保留
System32、`%USERPROFILE%\.cargo\bin`、Git cmd）→ `call vcvars64.bat` → `%*`。
所有 cargo 命令通过 `cmd /c "…\msvc.cmd cargo …"` 执行（`MSYS_NO_PATHCONV=1`）。

`tauri dev` 用同样的最小环境，但还必须能找到 node/python/ffmpeg，因此仓库新增 `scripts/dev.cmd`（L010 后补）：先用 `%%~dp$PATH:i` 从当前 PATH 发现 `node.exe`/`python.exe`/`ffmpeg.exe` 目录，再套用最小 PATH + `vcvars64.bat`（经 `vswhere` 定位 VS）；用法 `scripts\dev.cmd npm run tauri dev`。⚠️ 用系统 PATH 直跑会把 JDK shim 与 Git `link.exe` 带进来，必须走包装。

另一个 Windows 专属编译修复已入库：`src-tauri/build.rs` 注入 comctl32 v6 manifest
（依赖树静态导入 `TaskDialogIndirect`，缺 manifest 时链接出的测试/主程序会在 `main` 前崩溃）。

### 7.3 真实门禁结果（L006 基线）

- `cargo fmt --all -- --check`：干净（含 L001–L004 遗留的 11 处 rustfmt 偏差，已应用）；
- `cargo clippy --all-targets -- -D warnings`：0 告警（1.94.1 与 1.98.1 均通过）；
- `cargo test`：**194 passed / 0 failed**（1.94.1 与 1.98.1）。
- `Cargo.lock` 已按 `time`/`local-time` feature 刷新（新增 `num_threads`、`libc`）——关闭 L003 的遗留风险。

CI 盲区：`rust-ci.yml` 只在 push/PR 到 `main` 时触发且跑在 ubuntu；工作分支 `dev` 从未触发 CI，
上述 4 个真实缺陷（文件日志层泛型、Windows manifest、chunk JSON 路径、AC-18 fixture）一直未被发现。

### 7.4 无 cargo 时的备用通道（保留，已非必需）

无 cargo 时可用 Node 端口脚本做语义校验（`scripts/port-checks.mjs`，当前 44/44；改动对应 Rust 逻辑需同步），
或用 npm 分发的官方 Rust 组件（rustc/rustfmt/clippy/wasm std，同一 commit `42212a5c4`）搭临时 sysroot，
对只依赖 std 的模块跑真实编译与单测（`rustc --test --target wasm32-wasip1` + `node:wasi` 的 `preview1`）。
两者都不能替代整仓 `cargo` 门禁（前者只证语义；后者不覆盖 tauri 依赖、主机构建与链接）。

## 8. L005 真实媒体干跑（2026-09-17，开发者指定 video `nIABz0Z4IRA`）

范围：用真实 yt-dlp/ffmpeg/whisper（GPU）+ 真 Rust 纯函数（wasm harness，见 §7）跑通“下载→探测→分块→转录→合并→英文原稿”。
这是**后端干跑**，不是应用级 E2E：L006/L008/L009 与 Phase B 尚未实现，且本机无 cargo 无法构建 Tauri 应用。

视频：`How to Learn So Fast People Assume You're Naturally Gifted`，时长 **502.224399s**（8m22s）。

### 8.1 实测结论

| 环节 | 结果 |
| --- | --- |
| 下载（AC-02 语义） | `-f ba/best` 需搭配 `--js-runtimes node --remote-components ejs:github --extractor-args "youtube:player_client=mweb"` 才成功；无 JS runtime 时 n challenge 失败、媒体 GET 返回 **HTTP 403**。本次 mweb 的纯音频格式因缺 GVS PO Token 被跳过，`ba/best` 按设计回退到 `best`（format 18，27.9MB 视频+音频） |
| 时长（L004） | `ffprobe -v error -show_entries format=duration -of json` → `502.224399` |
| 分块（AC-05） | 真 Rust `plan_chunks`：默认 20min → 1 块 `[0,502.224]`；3min → `[0,180] [180,360] [360,502.224]` |
| 切块（L004 的 `-t` 偏差） | `-ss <start> -i <in> -t <end-start> -c copy`；实测块长 180.001088 / 180.001995 / 142.223991，与计划跨度一致 |
| whisper stdout 契约（AC-03） | 每块 stdout 段行数 == JSON `segments` 数（68/67/33，全部 MATCH），`language=en`；`small` 首次自动下载 |
| 真 Rust 合并（AC-07） | 168 段 = 68+67+33（零丢失），`covered=502.224`，无 coverage gap；`boundary_risks` 命中 chunk 0/1（tail+head 均为 true） |
| 真 Rust 原稿（AC-09） | 28 段 / 8843 字符；与整片单块转录（209 段 / 33 段 / 8841 字符）做词级 difflib：**相似度 0.9933**，10 处差异均为 ASR 抖动（`gonna`/`going to`、`cause`/`because`、漏听 `I'll`、`otherwise`/`lois`），**无边界丢词**：切点句 “you think it would help you | be less confused” 两块合起来完整（同理 “figure | it out”） |
| 缺失块路径（AC-07 告警） | 人为去掉 chunk 1 的段：`segments=101`、`covered=322.224`、`COVERAGE_GAP chunk=1 gap=180.000`，其余段零丢失 |
| 性能（RTX 5050 8GB，`small`+fp16） | 3min 块 17–22s/次（含模型加载）；502s 整片 49s，约 **10× 实时**；运行时空闲显存约 7.2GB |

### 8.2 真实边界告警观察

两处内部边界都报了 `chunkBoundaryRisk`（tail+head）：

- head 恒真：whisper 对每块总有一个 `start=0.0` 的首段；
- tail 恒真：块末段会越过切点（实测 chunk 0 末段 `end=180.20` > 切点 `180.001`），合并侧按 span 截断覆盖但不丢文本。

即按现有阈值，**几乎每个句中切点都会告警**（warning-only、保留现场，符合设计）；若 Phase C 验收认为噪声过大，可在评审时讨论调参（例如要求 tail+head 同时命中、或对重叠音频做词级校验），本期不改规则。

### 8.3 待定/风险（已同步 design Risks）

- **YouTube 现行下载要求**：yt-dlp 2026.07 起对 YouTube 需 JS runtime + EJS 求解脚本，否则 403；`--remote-components ejs:github` 会在运行时从 GitHub 取脚本，与“远端内容必须签名校验”的架构规则冲突，需在 L008/Phase C 前定方案（签名清单分发 JS runtime+求解脚本 / 依赖用户 Cookie / 固定可用 client）。
- 干跑产物留在 `E:\tmp\ytb-e2e\`（`transcript_chunked.txt`、`transcript_whole.txt`、各块 JSON/stdout/stderr），未进入仓库；应用级 L3 验收仍需等 L006–L009 + Phase B。

## 9. 翻译模块的持久性事实（L007）

### 9.1 分块与 prompt（`translation/blocks.rs`）

- `plan_blocks`：连续段落成块，上限 `maxSegmentsPerBlock`（默认 6）且 `maxCharsPerBlock`（默认 3000）；**段落永不切分**（Q6 的 1:1 对齐），超长段独占一块；配置为 `0` 时钳为 `1`，不会产生空块/无限块。
- `parse_glossary`：每行首个 `=` 分割，两侧 trim 后均非空才采纳；空行/无 `=`/空边一律跳过（不报错）。
- prompt 全部集中在 `blocks.rs`：`build_system_prompt`（角色 + 硬规则 + `dropFillers` 决定是否加去填充词条目 + 术语表 + JSON 输出契约）与 `build_user_prompt`（前 2 段的“原文 + 已产出译文”上下文 + `[id] text` 列表）。response id = 全局段落索引（0-based），与 `zh.blocks.json` 的 id 一致。

### 9.2 校验与数字保真（`translation/validate.rs`）

- 契约顺序：先查 id 集合（缺 id → `MissingId`，多余 → `UnexpectedId`），再查数量（重复 id 导致数量不符 → `WrongItemCount`），最后逐位查顺序与 `zh.trim()` 非空；任一项不过 → `translationContractViolation`，不落盘。`TranslationItem` 用 `deny_unknown_fields`，模型额外添加的字段/说明文本会在反序列化阶段就被拒绝。
- `extract_numbers` 保留数字、数字间 `.`/`,` 与尾随 `%`（`2,024`、`3.5%`）；保真比对先归一化（去空格、`,` 千分位、`％`→`%`），再做**数字边界**检查（`190%` 不会满足 `90%`）；命中术语表映射（源词含该数字且译文用了术语表译文）也算通过。只告警，不影响落盘。

### 9.3 HTTP 与重试（`translation/deepseek_client.rs`）

| 情形 | 行为 |
| --- | --- |
| 401 / 403 | `AuthFailed`，不重试（`deepseekAuthFailed`） |
| 429 | 退避后重试：优先 `Retry-After`（秒，封顶 60s），否则指数退避（1s 起、每次翻倍、封顶 8s） |
| 5xx | `ServerError { status }`，可重试（`deepseekServerError`） |
| 其它 4xx | `RequestRejected { status }`，不重试（错误码由 L008 决定，候选 `deepseekServerError`） |
| 超时（120s）/ 网络 | `Timeout` / `Network`，可重试 |
| 响应 JSON 或契约不符 | `InvalidResponse`，可重试（模型下次可能给出正确 JSON） |

- 尝试次数 = `maxRetries + 1`（`maxRetries` 为 2 时最多 3 次请求）；`with_base_delay(Duration::ZERO)` 是给 L1/L2 测试的接缝。
- key 每次请求由调用方传入（L008 每块从 stronghold 读一次，不缓存），只写入 `Authorization: Bearer …` 头；`DeepseekClient` 不持有 key，也不记录任何请求/响应明文（只回传 `usage`）。
- L1 用自建 `std::net::TcpListener` mock（无新 dev-dependency）覆盖：401 不重试、429 退避后成功、5xx 重试上限、契约失败重试/耗尽、请求体（`model`/`temperature=0.3`/`json_object`/system+user）与“key 只在头不在体”。真实 DeepSeek 调用属 Phase C L3（需真 key）。

## 10. 流水线与产物的持久性事实（L008）

### 10.1 模块与并发

| 模块 | 职责 |
| --- | --- |
| `transcribe/artifacts.rs` | 原子写（`.tmp`→rename）、完整性/损坏判定、产物路径与目录名消毒、`source.json` 标记、`summary.md` 之外的 `.work` 读写（segments/zh.blocks）、组装 zh 原稿、用量合计 |
| `scheduling/transcribe_pipeline.rs` | 批/视频编排、四阶段、事件、取消、限流、清理、批次汇总 |
| `models/transcribe.rs` | IPC 载荷（stage/progress/artifact/batch_summary）与 `VideoStatus`/`VideoErrorCode`（与 design 错误码表逐字一致） |

- 并发模型：`GenericDispatcher` 的信号量**直接复用 `DownloadLimiter`**（即“视频槽”，默认 2），因此“下载并发 ≤2”天然成立（每个视频在任一时刻只下一个音频）；视频内 whisper 用 `TranscribeLimiter`（固定 1）串行，翻译每个 block 取 `TranslateLimiter`（= `translation.concurrency`，启动时读取，与现有 `DownloadLimiter` 一样不支持热改）。
- 阶段与产物：`downloadingAudio`（fetch 元数据 + 下载音频）→ `transcribing`（时长→切块→whisper→merge→写 `transcript.en.txt`）→ `translating`（逐 block，写 `zh.blocks.json`）→ `writing`（拼 `transcript.zh.txt` + 清理 `audio.*`）。成功发 `media_complete`，失败发 `media_fatal`（`internal=false`，`details=错误码`）。
- 跳过/续跑：① 本地 `source.json` 命中两份 txt → 零网络跳过（AC-14，见 design.md 新增 Deviation）；② fetch 后若 `segments.json` 完整则复用（跳过 whisper）；③ 仅在“复用了转录且 `zh.blocks.json` 与当前 block 计划完全匹配”时才跳过翻译（重转录会作废旧译文）；`overwrite=true` 全部重跑。
- 失败保留现场：任何失败/取消都不删音频、不删中间产物；只有两份 txt 都写成功后才按 `keepAudio` 决定是否删 `audio.*`（分块文件保留以便人工校对）。
- 批次汇总：`register_batch` → 每个视频上报 `BatchSummaryItem`（状态/错误码/产物/跳过/用量）→ 最后一个上报时原子写 `<root>/summary.md`（表格 + Totals）并发 `batch_summary`，随后移除批状态（清理时机明确）。

### 10.2 错误码映射（L1 单测覆盖）

| 来源 | 映射 |
| --- | --- |
| `WhisperError::OutOfMemory` / `SpawnFailed` / 其余 | `whisperOutOfMemory` / `whisperMissing` / `whisperFailed` |
| `FfmpegError::SpawnFailed` / 其余 | `ffmpegMissing` / `ffmpegChunkFailed`；`DurationUnknown` 由时长探测失败分支产生 |
| `DeepseekError::AuthFailed`/`RateLimited`/`Timeout`/`InvalidResponse`/其余 | `deepseekAuthFailed`/`deepseekRateLimited`/`deepseekTimeout`/`translationContractViolation`/`deepseekServerError`（`RequestRejected` 也归 `deepseekServerError`） |
| 本地写盘/元数据/下载 | `outputWriteFailed` / `fetchFailed` / `downloadFailed` |

### 10.3 遗留与风险

- **L2 未跑**：AC-15/16/19 的联动判定需要 fake whisper/ffmpeg + mock DeepSeek + 真实文件系统（Phase C 第 18 项）；本循环只到 L1 + 编译/clippy/fmt。
- 跳过路径不发 `media_add`（卡片生成交给 L009 的命令响应 / Phase B）。
- 同标题视频会落到同一目录（设计既有命名方案的固有风险），已有 `source.json` 可辅助识别但不解决冲突；如需要可在后续循环加目录后缀。
- 流水线模块在 L009 接线前曾挂模块级 `#[allow(dead_code)]`，L009 已全部删除（见 §11.4）。

## 11. 命令接线与 `media_size` 移除（L009）

### 11.1 `transcription_probe`

- 字段：`whisperPath`/`whisperVersion`/`whisperFound`、`cudaAvailable`/`cudaDevice`、`model`/`modelCached`/`modelPath`/`modelSizeBytes`、`ffmpegPath`/`ffprobePath`、`apiKeyConfigured`（只读 stronghold，不验连通性，C3）、`logDir`/`logFile`/`logSizeBytes`（AC-27）。
- 程序解析 `resolve_program(configured, bin_dir, name)`：显式配置（`transcription.whisperPath`）必须存在；否则按 `bin_dir` → `PATH` 查找，Windows 依次尝试 `.exe/.cmd/.bat`。
- 探测细节：whisper 版本来自身`python -m pip show openai-whisper`（CLI 无 `--version`）；CUDA 用 `nvidia-smi --query-gpu=name`（驱动级，不加载 torch，避免探针卡 10s+）；模型缓存路径 `~/.cache/whisper/<model>.pt`（模型名 small/medium/large-v3）；所有外部命令经 `spawn_blocking` + `configure_command`（无窗口）。
- 探针同时写文件日志 `probe.ok`/`probe.missing`（AC-24）。

### 11.2 `transcribe_start` 与门禁

- 流程：清洗/去除空 URL → 非空校验 → `whisper_found(app)` 门禁（不 spawn，只看解析结果；缺失返回 `Err("whisperMissing")`，AC-01）→ 取 `output.rootDir` → 每条 URL 生成 `group_id`/`id` 并 `ensure_group_running` → `TranscribeRequest::Batch` → 返回 groupId 列表；同时写 `run.start` 日志。
- 播放列表自动展开（design §2）在 L010 补上：fetch 阶段解析出 `ParsedMedia::Playlist` 后由 pipeline 展开为同 group 的逐视频任务（接口、计数与事件见 §12.1）。

### 11.3 `media_size` 移除清单（已全部完成）

- 后端：命令文件、`lib.rs` 的 `invoke_handler` 条目、`FetchRequest::Size/SizePlaylist` 与其展开/计数分支、`FetchEntry.format`、`MediaAddWithFormatPayload`。
- 隔离层：`src-isolation/main.ts` 白名单条目（同时补上 `transcribe_start`/`transcription_probe`）。
- 前端：`src/stores/media/size.ts`、`stores/media/media.ts` 的引用、`listeners/media.ts` 的 `media_size` 订阅、`types/media.ts` 的 `MediaAddWithFormatPayload`、`MediaConfigureStep.vue` 的体积展示与加载按钮（Phase B 会整体替换该步骤；相关 i18n 键暂留待 Phase B 清理）。

### 11.4 死代码豁免清理结果

全部移除：`transcribe/*`、`translation/*`、`scheduling::transcribe_pipeline`、`runners::{ffmpeg,whisper}_runner`、`ytdlp_process::{ProcessResult,run_streaming,tail_excerpt}`、`ytdlp_args::audio_args`、`logging::events`、`file_log` 的 3 处、`stronghold_state` 的 6 处。清理后 clippy 暴露的真实缺口已补齐：`probe.ok/missing`、`audio.download.fail`（含 exit/errCode）、`translate.block.retry`（客户端内按 attempt/status 记录）；pipeline 改用 `plan_skips`/`ArtifactState` 做续跑判定。仅 `DeepseekClient::with_base_delay` 保留 `#[allow(dead_code)]`，注释说明它是 L1/L2 试验接缝。

## 12. 播放列表展开、批次通知与取消清理（L010）

### 12.1 播放列表展开（design §2）

| 维度 | 决定 |
| --- | --- |
| 落点 | `scheduling/transcribe_pipeline.rs` 的 fetch 阶段：`ParsedMedia::Playlist` 不再报 `fetchFailed`；`expand_playlist` 在同一 group 内生成逐视频任务，并通过 `TranscribeRequest::Expand` 追加进正在运行的批次 |
| group 语义 | 播放列表链接 = 1 个 group（`transcribe_start` 返回的 id 即该 group），每个可用条目 = 该 group 的一个视频 item；`TranscribeEntry.total` 携带组内视频数，子项的 `media_add.total = N` |
| 事件 | 先发播放列表 leader（`media_add` 的 `item = ParsedPlaylist`，前端可拿 group 标题/条目），再逐个发子视频 `media_add`；新增文件日志事件 `playlist.expand`（INFO：run/group/entries） |
| 批计数 | `expand_batch(batch_id, children)` 把尚未上报的链接任务槽替换为 N 个子视频（`remaining - 1 + N`）；链接任务返回 `None`（`run_video -> Option<VideoOutcome>`），不产生汇总行 |
| 续跑 | 每个子视频照常走 AC-14 的本地 `find_existing_output`（子项零网络跳过）；链接本身仍需一次 `-J --flat-playlist` 才能发现条目 |
| 取消 | 子视频继承链接 group id，`group_cancel` 对整条播放列表生效（见 §12.3/§12.4） |
| 测试 | `playlist_children_*`（顺序/同 group/去空 URL/`total`）、`flat_playlist_fixture_*`（固定 `-J --flat-playlist` JSON → `parse_ytdlp_info` → 展开）、`expanding_a_playlist_replaces_the_link_slot_in_the_batch` |

### 12.2 `batchFinished` 通知（Phase A 第 10 项）

| 维度 | 决定 |
| --- | --- |
| 代码分层 | 新增根模块 `src-tauri/src/notifications.rs`（`NotificationKind` + 泛型 `notify<R: Runtime>`）；`commands/notifications.rs` 变薄壳；`state/config_models.rs` 与 `scheduling/transcribe_pipeline.rs` 依赖该模块，不再反向依赖 `commands` |
| 触发点 | `transcribe_pipeline::record_outcome` 在批次归零、写 `summary.md`、发 `batch_summary` 之后调用 `notify(BatchFinished)`；成功/失败/混合批次都只通知一次 |
| 参数 | `done`/`failed`/`skipped` 三个计数（body 不用 `n` 复数选择，避开 12 种语言的复数规则） |
| key 契约 | 后端 `src-tauri/locales/*.json`：`notifications.batchFinished.title|body`；前端 `src/locales/*.json`：`settings.notifications.disabled.kinds.batchFinished`（13 个语言文件全量补齐） |
| 测试 | `notifications::tests::every_kind_has_a_backend_and_frontend_locale_entry`（逐 kind 校验两份 en locale）、`batch_finished_keys_follow_the_notification_contract`、`batch_notification_params_*`、`the_last_outcome_writes_the_summary_and_drops_the_batch`（mock app；无 `SharedConfig` 时 `notify` 经 `try_state` 静默跳过） |

### 12.3 取消路径的已知缺口（W009，Phase C 修复）

`GenericDispatcher` 在 `group_cancel` 后会把该 group 尚未派发的队列条目静默丢弃（`queues.retain` 与 `is_group_running` 分支），这些条目不会调用 `record_outcome`；批计数按条目推进，因此**在队列中取消**会让该批永不归零：`summary.md` 不写、`batch_summary`/`batchFinished` 不发。运行中的条目正常上报 `cancelled`。Phase C 的 AC-16 L2 取消测试需要先修此缺口（候选：pipeline 按 group 记录未上报条目、取消时合成 `Cancelled` outcome；或让 dispatcher 上报被丢弃的条目）。

### 12.4 `group_cancel` 清理

`commands/group/group_cancel.rs` 现在也向 `TranscribeSender` 发 `DispatchRequest::Cleanup`，与 fetch/download 一致：取消后移除 `RUNNING_GROUPS` 分组并清掉排队条目（不改运行中任务的取消语义）。

## 13. Phase B 前端基础（L011）

### 13.1 `transcription` store（`src/stores/transcription.ts`）

| 维度 | 决定 |
| --- | --- |
| 职责 | 仅会话内（不持久化）：probe 结果、逐 item 阶段、块/翻译进度、artifact 路径、token 用量、批次汇总 |
| 阶段→状态 | `transcribe_stage` 经 `stageStates` 映射到 `MediaState.{downloadingAudio,transcribing,translating,writing}` 并写入 `media-state` store（与下载进度同一模式，进度驱动状态） |
| 进度 | `transcribe_progress` → `chunkProgress[id]`（percent 钳 0..100）；`translate_progress` → `blockProgress[id]` + 累计 `usage[id]`；`usageForGroup` 经 group store 聚合 |
| 产物/汇总 | `artifact_written` 按 `kind` 合并 `{en, zh}`；`batch_summary` 追加到 `summaries`，`latestSummary` 取最后一条 |
| 清理 | `forgetItem(id)`（删除分组时用）与 `reset()`；五个监听在 `src/tauri/listeners/transcription.ts`，注册于 `plugins/tauriListeners.ts` |

### 13.2 卡片步骤（Phase B 第 11 项增量）

- `MediaState` 新增 `downloadingAudio`/`transcribing`/`translating`/`writing`（`configure` 暂留，删除随第 13 项的首页输入改线，避免下载入口先失效）。
- 新组件：`AudioDownloadStep`（indeterminate，音频下载无字节级进度）、`TranscribeStep`（整体百分比 = 块内 percent 折算 + 当前模型）、`TranslateStep`（块完成度 + 累计 token）。
- `MediaCard.vue` 的 `stepMap` 接入四态；`writing` 复用 `TranslateStep`（design 的步骤清单没有 Writing 组件）。
- i18n：`media.steps.{audioDownload,transcribe,translate}` 已补 `en` 与 `zh-CN`，其余语言留给第 17 项。
- 步骤组件按 group 的第一个 item 读取进度；playlist group 拆分（第 13 项）后即为 1:1。

## 14. 环境检查页与输入门禁（L012）

| 维度 | 决定 |
| --- | --- |
| probe 动作 | `transcription` store 增 `runProbe()`（`invoke('transcription_probe')`），失败保留上一次结果；`isProbeLoaded`/`isEnvironmentReady` 派生 |
| 必需项 | `whisperFound && ffmpegPath && ffprobePath`（AC-01 门禁 + 分块依赖）；CUDA、模型缓存、DeepSeek key、日志路径只展示不阻断 |
| 启动探测 | `main.ts::initStores` 启动时调一次 `runProbe()`（design §8 的“启动与手动重检”）；`/setup` 挂载再探测一次，页内“重新检测”可手动重跑 |
| 页面 | `src/views/app/SetupView.vue` + 路由 `/setup`（`name: 'setup'`），展示 whisper/CUDA/模型/ffmpeg/ffprobe/key/日志路径与大小；`en`+`zh-CN` 的 `setup.*` 键 |
| 门禁行为 | `TheHeader`：probe 已加载且必需项缺失时，输入与 Add 禁用、placeholder 改为提示，提交改为 toast + 跳 `/setup`；probe 未加载时不阻断（后端 `transcribe_start` 仍是最终门禁） |
| mock | `tests/utils/mocks/transcriptionHandlers.ts`（`readyProbe` + `transcription_probe` handler），注册进 `main.ts` 的 E2E mock 组合 |
| 测试 | store：`runProbe` 调用与 `isEnvironmentReady` 门禁（含 whisper 缺失）；E2E：`tests/e2e/setup.spec.ts` 断言 probe 渲染；`header.spec.ts` 补 `setup` 路由 |
| 未做（L013） | 首页输入仍是 `media_info`/下载向 `addUrlBatch*`；`transcribe_start` 接线、`configure`/`MediaConfigureStep` 删除、下载向 E2E 退场留到 L013（与流程替换同轮，避免入口空缺） |

## 15. 首页输入改接转录链路（L013）

| 维度 | 决定 |
| --- | --- |
| 入队 | `media` store 新增 `startTranscriptionBatch(urls)`：invoke `transcribe_start` → 每个返回的 groupId 建一个 `transcribeMode` group（含一个占位 item）+ `fetching` 状态；`TheHeader`（输入/剪贴板/文件导入）统一走它，shift-click 的“立即下载”语义删除 |
| 事件接线 | `media_add` 仍走 `processMediaAddPayload`：`transcribeMode` 下 leader 只记录播放列表元数据并停在 `fetching`（不触发选择 UI）；子项集齐（`processed === total`）→ `splitTranscribedGroup` 用现有 `splitGroup` 按 URL/条目顺序拆成“一视频一卡片”，并继承 `transcribeMode` |
| 阶段 | `transcribe_stage` 由 `transcription` store 映射到 `MediaState.{downloadingAudio,transcribing,translating,writing}`（L011），卡片步骤随之切换 |
| 跳过路径 | 跳过时后端不发 `media_add`（AC-14 本地判定）；`processMediaCompletePayload` 在 `payload.id` 不属于该 group 时把 group 的占位 leader 置为 `done` |
| 卡片动作 | `transcribeMode` 卡片隐藏下载/暂停/恢复，只保留删除（后端 `group_cancel`）、外链、错误重试（`retryTranscriptionGroup` 重建批次）；metadata 入口保留 |
| 测试 | 单测 `transcriptionFlow.spec.ts`（批次创建 + 播放列表拆分）、`header.spec.ts`（输入 → `startTranscriptionBatch`、环境门禁禁用按钮 + 提交跳 `/setup`）；E2E `transcribe-flow.spec.ts`（add → 转录/翻译步骤与 token）；下载向 E2E 退场：删除 `download-progress`/`global-selection`/`group-behaviour`/`playlist-selection`/`persist-selection`/`queue-actions` 六个 spec（对应 UI 属第 16 项删除范围） |
| 未做 | `configure`/`MediaConfigureStep` 与下载向 helpers/组件/单测的物理删除留到第 16 项（同一删除面）；`media_info`/`media_playlist_expand` 命令仍在后端但前端已无入口 |

## 16. 设置页签与模型配置（L014）

| 维度 | 决定 |
| --- | --- |
| 页签 | `settings` 改为 转录 / 翻译 / 输出 / 网络 / 系统 / 关于（默认 `settings.transcription`）；`SettingsDownloadsTab`/`SettingsAppTab` 删除，外观/通知/输入/更新并入系统页；顶栏设置链指向 `settings.transcription` |
| 转录页 | `SettingsTranscription.vue`：model（small/medium/large-v3）、device（cuda/cpu）、fp16、language（en/auto）、chunkMinutes（0=不分块）、keepAudio、conditionOnPreviousText、whisperPath 覆盖 |
| 翻译页 | `SettingsTranslation.vue`：`ai.apiKey`（password + 保存 + 已配置/未配置徽标）、baseUrl、model、temperature、concurrency、maxRetries、dropFillers、术语表 textarea |
| API key 写入 | `stronghold` store 新增 `setAiApiKey(value: string \| null)`：只写/删 `ai.apiKey`（`null` 删除），**从不读回**（`getValues` 仍只覆盖 auth 字段）；“是否已配置”由 `transcription_probe.apiKeyConfigured` + 本次会话的保存标记展示，保存后重跑 probe 刷新；写入前若 vault 未解锁先 `loadStatus` → `stronghold_init`（全新安装没有 `vault.hold`，启动自动解锁不会发生，否则直接写会报 `vault locked`） |
| 保存反馈 | 保存成功后清空输入框（不保留明文），但显示持久反馈：绿色 badge（已配置）+ 内联成功提示（用户开始输入新密钥时才隐藏）；提供“清除密钥”（带确认） |
| 输出页 | `SettingsOutput.vue` 重写为 rootDir / overwrite / restrictFilenames（旧的 video/audio/模板/后处理字段不再展示） |
| i18n | `settings.tabs`/`settings.transcription`/`settings.translation`/`settings.output` 已补 `en` + `zh-CN`；其余 11 种语言暂缺（vue-i18n 回退 en，翻译补齐属后续收尾） |
| 删除 | `SettingsDownloadsTab.vue`/`SettingsAppTab.vue`；`tests/unit/postprocessSettings.spec.ts`/`postprocessOverrides.spec.ts`（针对已移除的后处理 UI） |
| 测试 | `settingsView.spec.ts` 改为新页签 + 用转录页复选框验证 save/reset；`header.spec.ts` 路由名同步 |

## 17. 详情页三 tab 与打开产物（L015）

| 维度 | 决定 |
| --- | --- |
| 路由 | `group.metadata` 移除，改为 `group.en`（默认）/ `group.zh` / `group.logs`（`TheTranscript` 通过 `props: (route) => ({ groupId, kind })` 区分） |
| 文稿 tab | `TheTranscript.vue`：展示 `artifactsFor(首个 item).{en,zh}` 路径 + “打开输出目录”（openPath 目录）+ “用系统程序打开”（openPath 文件）；未落盘时显示空态；**不做内嵌预览**（裁剪 C4，无 `artifacts_read`） |
| 日志 tab | 保留内存诊断/日志流；新增“打开日志文件”按钮，路径来自 `transcription_probe.logFile`（AC-27）；前端不读日志内容 |
| 卡片入口 | `MediaCardActions` 的信息按钮从 `group.metadata` 改指 `group.en` |
| i18n | `media.view.tabs.{en,zh}`、`media.view.transcript.*`、`media.view.logs.openFile` 补 `en`+`zh-CN` |
| 测试 | 新增 `theTranscript.spec.ts`（路径展示 + 两个 opener 调用 + 未生成空态） |
