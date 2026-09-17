---
status: current
layer: domain
domain: download-engine
canonical_for:
  - download-engine-progress-and-diagnostics
related:
  - docs/current/platform/observability.md
  - docs/current/domains/media-queue/error-handling.md
last_verified: 2026-09-17
---

# 进度解析与诊断规则

## Purpose

把「yt-dlp 输出的哪一行会被解析成什么事件」「诊断码从哪来、如何扩展」写成可维护契约，
避免在解析器里堆叠临时字符串匹配。

## Current Behavior

### 进度解析（`parsers/ytdlp_progress.rs`）

`YtdlpProgressParser` 持有 `id`、`group_id`、`current_category`、`current_stage`、`partial_download_duration_secs`。
`parse_line` 按固定顺序尝试（命中即返回）：

| 顺序 | 匹配 | 产出 |
| --- | --- | --- |
| 1 | `[download] <path> has already been downloaded` | `Destination`（confidence 65） |
| 2 | `[VideoRemuxer] Not remuxing media file "<path>"` | `Destination`（confidence 90） |
| 3 | 含 `Destination:` 的行 | `Destination`（confidence 按标签，见下） |
| 4 | `[VideoRemuxer]` / `[VideoConvertor]` 前缀 | `StageChange` → `remuxing` / `reencoding` |
| 5 | `[download] Destination: <path>` | 按扩展名更新 `category`；首次出现时 `StageChange` → `downloading` |
| 6 | `RAW\|…` | `Progress` |
| 7 | ffmpeg `frame= … time=… speed=…` | `Progress`（仅部分下载时；首次会同时把阶段从 `initializing` 推为 `downloading`） |
| 8 | `[Merger] Merging formats into "<path>"` | `Destination`（`isMerged=true`，confidence 80） |
| 9 | 以 `[Merger]` 开头 | `StageChange` → `merging` |
| 10 | 含 `[ffmpeg]` / `[Fixup]` / `Deleting original file` | `StageChange` → `finalizing` |

`Destination` 标签 → confidence：`VideoConvertor` 95、`VideoRemuxer` 90、`Merger`/`ExtractAudio` 70、`download` 60、其它 50（`[Merger]` 前缀另有 80 的独立分支）。

`RAW|` 行的 9 个字段（与 `--progress-template` 一一对应）：

| # | 模板字段 | 解析用途 |
| --- | --- | --- |
| 0 | `progress.percent` | 百分比（首选） |
| 1 | `progress._percent_str` | 百分比（次选，去掉 `%`） |
| 2 | `progress.speed` | 字节/秒 |
| 3 | `progress.eta` | 剩余秒 |
| 4 | `progress.downloaded_bytes` | 与 5/6 组合估算百分比 |
| 5 | `progress.total_bytes` | 同上 |
| 6 | `progress.total_bytes_estimate` | 同上 |
| 7 | `progress.fragment_index` | 与 8 组合估算百分比 |
| 8 | `progress.fragment_count` | 同上 |

百分比优先级：`percent` → `_percent_str` → `downloaded/total|estimate` → `fragment_index/count`；
结果 clamp 到 `0..100`；全部为空且无速率与 ETA 时**丢弃该行**（不产生事件）。

类别（`ProgressCategory`）由 `[download] Destination:` 的扩展名决定：
视频 `mp4/mkv/webm/flv/mov/avi/m4v/ts/m2ts/3gp`；音频 `mp3/m4a/wav/flac/ogg/opus/mka/aiff/wma/alac/aac`；
字幕 `vtt/srt/ass/lrc/ttml/srv1/srv2/srv3`；缩略图 `jpg/jpeg/png/webp/bmp`；元数据 `json`；其它 `other`。
初始类别由 `progress_category_for_track_type`（`both/video` → `video`，`audio` → `audio`）给出。

### 阶段（`ProgressStage`）

`initializing`（初始）→ `downloading`（首个 `Destination:` 或 ffmpeg 进度）→ `merging`（`[Merger]`）
→ `remuxing` / `reencoding`（后处理器）→ `finalizing`（`[ffmpeg]`/`[Fixup]`/删除原文件）。
阶段只前进（`current_stage` 不会回退），同一阶段重复出现不重复发事件。

### 诊断规则文件（`src-tauri/src/diagnostic_rules.json`）

结构：

```json
{
  "fallbackCode": "unknown",
  "rules": [
    {
      "code": "signInRequired",
      "appliesTo": "error",
      "component": "ffmpeg",
      "patterns": [
        { "kind": "substr", "value": "Sign in required" },
        { "kind": "regex", "value": "not a bot" }
      ]
    }
  ]
}
```

- `appliesTo`：`error` / `warning`（必须与行前缀一致，否则跳过该规则）。
- `component`：可选；指定后仅当行内组件标签（`[ffmpeg]` → `ffmpeg`）相等才匹配。
- `kind`：`substr`（大小写不敏感包含）或 `regex`（`regex` crate 语法）。
- 匹配顺序 = 文件顺序，**先命中先返回**；都不命中返回 `fallbackCode`（`unknown`）。
- 另有解析器内置的 `input_filter_skipped`（不走规则文件），用于体积/日期/匹配过滤导致的跳过。

### 全部诊断码

| code | 级别 | 组件 | 含义 |
| --- | --- | --- | --- |
| `signInRequired` | error | — | 需要登录/年龄验证/私有视频 |
| `signInRequiredForBotDetection` | error | — | 触发机器人校验（"not a bot"） |
| `membersOnly` | error | — | 会员专属内容 |
| `geoBlocked` | error | — | 地区限制 |
| `accessForbidden403` | error | — | HTTP 403 |
| `notFound404` | error | — | HTTP 404 |
| `server5xx` | error | — | 500/502/503/504 |
| `rateLimited429` | error | — | 触发限流 |
| `requestedFormatUnavailable` | error | — | 指定格式不可用 |
| `unableToExtractInitialData` | error | — | 无法解析站点初始数据 |
| `unableToExtractIdOrInfo` | error | — | 无法提取 id/信息 |
| `nsigExtractionFailed` | error | — | nsig 解算失败（YouTube 常见） |
| `embedOnly` | error | — | 仅允许嵌入播放 |
| `urlUnsupported` | error | — | 站点不支持该 URL |
| `unknownUrlType` | error | — | URL 类型未知 |
| `networkNameResolutionFailed` | error | — | DNS 解析失败 |
| `playlistPrivateOrMissing` | error | — | 播放列表私有或不存在 |
| `videoNotFoundOrPrivate` | error | — | 视频不存在/私有 |
| `channelUnavailable` | error | — | 频道不可用 |
| `ffmpegNotFound` | error | ffmpeg | 找不到 ffmpeg |
| `ffmpegFailed` | error | ffmpeg | ffmpeg 执行失败 |
| `cannotAllocateMemory` | error | ffmpeg | ffmpeg 内存不足 |
| `permissionDenied` | error | — | 权限被拒 |
| `noWritePermission` | error | — | 目标目录不可写 |
| `windowsFileMissing` | error | — | Windows 文件缺失/被占用 |
| `blockedGeneric` | error | — | 通用拦截 |
| `copyrightClaimed` | error | — | 版权声明导致不可下载 |
| `notPremieredYet` | error | — | 首播未开始 |
| `livestreamNotStarted` | error | — | 直播未开始 |
| `sslVerifyFailed` | error | — | TLS 证书校验失败 |
| `sponsorBlockUnreachable` | error | — | SponsorBlock API 不可达 |
| `unableToConnectToProxy` | error | — | 代理连接失败 |
| `downloadRetryingFragment` | warning | — | 分片下载重试中（非致命） |
| `input_filter_skipped` | warning | download | 被体积/日期/匹配过滤跳过（解析器内置） |
| `unknown` | error | — | 未命中任何规则 |

### 前端消费约定

- i18n key：`errors.runner.<code>.message` 与 `errors.runner.<code>.shortMessage`；缺 key 时回退后端 `message`。
- 可上报（Sentry）条件：`level == 'error'` 且 `code === 'unknown'`。
- 跳过统计：`countSkippedDiagnostics` 只统计 `code === 'input_filter_skipped'`。

## Boundaries

- 规则只能基于单行文本匹配（没有跨行上下文），因此需要多个规则覆盖同一根因的不同文案。
- 解析器是纯函数式状态机（除阶段/类别两处内部状态），可在测试中直接构造行序列断言。
- 进度解析与日志写入互不影响：写日志失败不会中断进度。

## Contracts

- 新增诊断码的完整步骤：改 `diagnostic_rules.json` → 在 `src/locales/en.json` 的 `errors.runner.<code>` 添加文案 → 在其它语言补翻译（可后补，回退英文）→ 在 `progress-and-diagnostics.md` 表格登记。
- `--progress-template` 是解析器的外部契约，改动需同时改 `with_progress_args()` 与本文件字段表。

## Failure And Edge Cases

- 文件名含 `Destination:` 字样时可能被误判为目标路径（低风险，属已知取舍）。
- 规则顺序影响结果：更具体的规则必须排在更宽松的规则之前（例如 `nsigExtractionFailed` 在 `unableToExtractInitialData` 之前）。
- 正则规则在加载时编译；非法正则会让整个规则文件失效，从而触发 `InvalidDiagnosticRules` 内部错误（应视为发布阻断问题）。
- 进度在 `initializing` 阶段可能长时间无输出（例如解析站点信息），UI 依赖 `fetching`/阶段文本而非百分比判断"在动"。

## Verification

- Rust 单测：`parsers/ytdlp_progress.rs`（进度/阶段/目标路径）、`parsers/ytdlp_error.rs`、`parsers/ytdlp_single/tests.rs`。
- 前端单测：`tests/unit/skippedDiagnostics.spec.ts`、`tests/unit/mediaProgress.spec.ts`、`tests/unit/mediaDestination.spec.ts`。
- 端到端：`tests/e2e/download-progress.spec.ts`（通过 mock 事件验证 UI 表现）。
