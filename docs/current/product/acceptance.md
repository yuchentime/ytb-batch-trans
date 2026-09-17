---
status: current
layer: product
domain: product
canonical_for:
  - product-acceptance
related:
  - docs/evals/feature-checklist.md
  - docs/current/domains/media-queue/verification.md
last_verified: 2026-09-17
---

# 验收基线

本文件给出**业务级**验收规则（用户可感知语义），是各领域 `verification.md` 的上位约束。
代码级验收标准（AC）写在对应 change 的 `verification.md` 中，两者不可互相替代。

## 入队与元数据

- A1 合法 `http(s)` URL 入队后必须先出现卡片，再逐步补齐标题/缩略图/时长；抓取失败不得让卡片消失。
- A2 非法 URL（非 http(s)、空串、纯文本）不得入队，也不得发起 yt-dlp 调用。
- A3 CSV/TXT 导入与拖放只接受 `http(s)` URL，去重后入队；被跳过的条目数量要如实提示。
- A4 播放列表在未做选择前不得占用用户未确认的抓取量：只有用户确认的范围才逐条抓取。
- A5 `is_live` 内容必须以可读错误结束，不允许静默卡在抓取中。

## 队列状态机

- A6 状态只能取 `fetching / fetchingList / playlistSelection / configure / downloading / downloadingList / paused / pausedList / done / error`；任何未定义状态都视为缺陷。
- A7 下载/暂停/恢复/删除/重试按钮的可用性必须与当前状态严格对应（`configure` 才能改偏好，`downloading*` 才能暂停，`paused*` 才能恢复，`done`/`error` 才能重试或删除）。
- A8 暂停必须真正终止底层进程（不允许"看似暂停但仍在跑"），恢复必须重新下发未完成条目且不重复下载已完成条目。
- A9 删除 group 必须清理该 group 的全部前端状态（状态/进度/体积/诊断/目标路径/选项），且不得影响其它 group。

## 下载正确性

- A10 选择的 `trackType`、分辨率、帧率、编码、音轨必须体现在最终文件名与容器上，或给出明确失败原因。
- A11 组级 override 只影响该 group；未覆盖字段必须继续使用全局设置。
- A12 播放列表编号（`playlist_index`、`autonumber`、`playlist_autonumber`）在拆分与合并两种模式下都必须唯一且可预期；开启倒序编号后仅影响 `playlist_index`。
- A13 部分下载（时间段/章节）必须同时禁用章节嵌入，并在 `precise_cuts` 开启时对声道做关键帧对齐。
- A14 字幕只在目标的 `subtitleInventory` 中确实存在时才请求；请求了不存在的语言不得报错，也不得下载其余语言的替代品。
- A15 输出路径必须落在用户配置的下载目录内（视频/音频可分别配置），文件名模板中的路径分隔符必须被替换，不允许目录穿越。

## 安全与凭据

- A16 账号密码、视频密码、Bearer Token、请求头只能出现在 `.hold` 加密快照/钥匙串与 yt-dlp 命令行中；日志、Sentry、前端持久化里不得出现明文。
- A17 前端只能调用 `src-isolation/main.ts` 白名单内的命令；未授权命令必须被拒绝。
- A18 二进制安装必须同时通过清单签名校验与文件 sha256 校验，任一失败都要放弃本次安装并保留错误阶段信息。

## 平台与更新

- A19 关闭窗口的行为必须符合 `system.trayEnabled` 与 `system.closeBehavior` 的组合；托盘未启用时关闭即退出。
- A20 portable/snap/MS Store 形态不得尝试应用自更新，也不得因更新失败阻断启动。
- A21 设置保存后立即生效（并发上限、快捷键、托盘、语言、自启动），无需重启应用。
- A22 语言切换后前端界面与后端产生的托盘/通知文案都必须切换到目标语言，缺失 key 时回退英文。

## 观测与可支持性

- A23 每个 group 的原始 yt-dlp 输出必须可查看；缓冲区超限时丢弃最旧数据而不是崩溃。
- A24 可识别错误必须映射为稳定 `code`（见 `diagnostic_rules.json`）；新增错误模式应通过改规则文件而不是散落的字符串匹配。
- A25 被过滤跳过的条目必须显示为「跳过」而非「失败」。

## Verification

- 领域级验证：`docs/current/domains/*/verification.md`。
- 交付前检查：`docs/evals/feature-checklist.md`、`docs/evals/bugfix-checklist.md`、`docs/evals/regression-matrix.md`。
- 影响面映射见 `docs/evals/regression-matrix.md` 的「改动 → 必测旅程」表。
