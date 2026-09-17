---
status: current
layer: rules
domain: security
canonical_for:
  - security-rules
related:
  - docs/current/domains/auth-secrets/backend-behavior.md
  - docs/current/platform/frontend-runtime.md
last_verified: 2026-09-17
---

# 安全规则

## 凭据

1. 账号密码、视频密码、Bearer Token、自定义请求头只能存于 `vault.hold`（stronghold）+ 系统钥匙串主密钥；
   `config.store.json`/`preferences.store.json`/localStorage/日志/Sentry 中不得出现明文。
2. 前端与后端之间传递机密使用字节数组（`number[]`），不得用明文字符串跨 IPC。
3. 日志只允许记录"是否使用敏感 flag"的布尔摘要（`RunLogSummary`）；任何新增日志点都必须遵守。
4. 诊断上报（Sentry）只允许发送用户确认后的诊断文本；不得附带凭据或 Cookie 内容。
5. 排查问题时不得索取用户的 `vault.hold` 或钥匙串内容。

## IPC 与前端边界

1. 前端可调用命令必须同时存在于 `lib.rs` 的 `invoke_handler` 与 `src-isolation/main.ts` 的 `allowedCommands`。
2. 新能力需要 Tauri 插件权限时，必须在 `capabilities/default.json` 中最小化授权（禁止 `shell:allow-execute` 之类宽泛权限）。
3. CSP 不得放宽到 `unsafe-inline`/外部脚本源；引入远端图片仍通过 `img-src` 白名单式开放（当前为 `http: https:`）。
4. 前端渲染远端文本（标题、描述、诊断消息、链接）必须转义或经 `useLinkify` 的安全处理；不得把远端内容直接当 HTML 插入。

## 外部输入

1. URL 入队前必须校验协议为 `http(s)`（`isValidUrl`）；外链打开仅允许 `https:`（capability 限制 + `MediaCardActions` 校验）。
2. yt-dlp 参数通过 `Command::args` 传递，禁止把用户输入拼进 shell 命令字符串。
3. 自定义 ffmpeg 参数必须经 `shlex::split` 校验后原样传入（不做 shell 解释）。
4. 文件名/目录模板渲染必须消毒 `/ \ |` 与控制字符，禁止目录穿越（`template_context.rs` 有回归用例）。
5. 归档解压必须拒绝绝对路径与 `..` 条目（`binaries_extractor.rs`）。

## 供应链

1. 二进制清单必须通过 minisign 验签（内置公钥）后才解析；签名失败必须放弃安装。
2. 下载产物必须校验 sha256；不匹配必须放弃并报 `download_verify`。
3. 应用更新走 `tauri-plugin-updater` 的签名校验（`tauri.conf.json` 中的 pubkey）。
4. 新增依赖必须评估许可证（AGPL-3.0 兼容）与维护状态；`npm run licenses` 生成的第三方许可证必须随包分发。

## 数据最小化

1. 不新增遥测（分析、崩溃以外的上报）；现有上报只有 Sentry（后端错误/前端异常 + 用户手动上报）。
2. 不收集用户 URL、下载历史、Cookie、账号；这些数据不得离开本机（yt-dlp 直连目标站点除外）。
3. 新增本地存储文件必须在本目录（`docs/current/platform/storage.md`）登记并说明用途与清理方式。

## 禁止项

见 `forbidden.rules.md`。安全类改动必须在 change 的 `verification.md` 中列出对应的 AC 与验证方式。

## Verification

- `docs/evals/security-checklist.md` 全项。
- 发布前核对：CI 公钥与应用内硬编码公钥一致；`manifest:verify` 通过。
- 代码层检查：`cd src-tauri && cargo test`（含凭据摘要与路径消毒用例）、`npm run test:unit`。
