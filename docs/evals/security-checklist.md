---
status: current
layer: eval
domain: evals
canonical_for:
  - security-checklist
related:
  - docs/current/rules/security.rules.md
last_verified: 2026-09-17
---

# 安全交付检查清单

适用于任何涉及凭据、IPC、外部输入、供应链与遥测的改动；发布前也要整表过一遍。

## 凭据与隐私

- [ ] 新增字段不包含机密；机密只进 `vault.hold` + 系统钥匙串。
- [ ] 未在日志/事件/Sentry/配置文件/localStorage 中出现明文凭据（用 `grep` 搜索新增字段名与 `password`/`token` 关键词）。
- [ ] 新增日志点遵守 `RunLogSummary` 风格（只记布尔/计数）。
- [ ] 未新增遥测；Sentry 上报内容经过确认不含用户数据（URL 除外且为已知取舍）。
- [ ] 排查/文档中未要求用户提供 `vault.hold` 或钥匙串内容。

## IPC 与前端

- [ ] 新命令已加入 `src-isolation/main.ts` 白名单；未放宽其它命令。
- [ ] `capabilities/default.json` 仅在必要时扩展，且范围最小（无 `shell:allow-execute`、无通配路径写权限）。
- [ ] CSP 未被放宽；未引入外部脚本/样式/字体源。
- [ ] 远端文本渲染使用文本插值或 `useLinkify`，无 `v-html`/`innerHTML`。
- [ ] 外链打开仍限制为 `https:`。

## 外部输入

- [ ] URL 校验仍在入口层（`isValidUrl`），未把校验下移到后端或省略。
- [ ] 所有 yt-dlp 参数通过 `Command::args`/结构体传递；无 shell 拼接。
- [ ] 自定义 ffmpeg 参数经 `shlex` 校验。
- [ ] 模板渲染保留路径消毒（`/ \ |` 与控制字符），未放宽。
- [ ] 归档解压仍拒绝绝对路径与 `..`；新增压缩格式时补充同等级校验。

## 供应链

- [ ] 新增依赖：许可证与 AGPL-3.0 兼容；`npm run licenses` 能生成条目。
- [ ] 清单验签与 sha256 校验路径未被绕过（含失败时"仍然安装"的短路）。
- [ ] 更新流程仍校验签名（`tauri.conf.json` 的 pubkey 未变更或已同步发布）。
- [ ] CI 公钥（`ED25519_PUB_KEY_HEX`）与应用内 `MANIFEST_PUB_KEY` 一致。

## 数据最小化与存储

- [ ] 新增本地文件已在 `docs/current/platform/storage.md` 登记，说明内容与清理方式。
- [ ] 新增数据不属于"本机不外传"承诺之外的内容（URL/历史/Cookie/账号）。
- [ ] 数据目录权限与 portable/snap/MS Store 形态兼容（可写）。

## Verification

- 手工：按上表逐项检查并在 PR 中声明；发现风险项必须记录处理方式。
- 自动化：`npm run test:unit`（路径消毒/凭据摘要用例）、`cd src-tauri && cargo test`。
- 发布前：结合 `release-checklist.md` 与 `manifest:verify`。
