---
status: current
layer: domain
domain: auth-secrets
canonical_for:
  - auth-secrets-entry
related:
  - docs/current/domains/download-engine/api-contract.md
  - docs/current/rules/security.rules.md
last_verified: 2026-09-17
---

# auth-secrets 文档路由

## Canonical Docs

- `flow.md`：保险库创建/解锁、字段读写、注入 yt-dlp 的完整流程。
- `api-contract.md`：`stronghold_*` 命令与密钥命名契约。
- `backend-behavior.md`：`StrongholdState`、主密钥、钥匙串、失败恢复策略。
- `frontend-behavior.md`：认证页、凭据表单、store、状态灯。
- `verification.md`：验证方式与安全回归清单。

## 范围

责任单元：**为需要登录/受保护的内容提供认证凭据，并把机密保存在应用之外（加密快照 + 系统钥匙串）**。
包含 Cookie（文件/浏览器）配置与机密字段（用户名、密码、视频密码、Bearer Token、自定义请求头）的存取，
以及它们在下载时注入 yt-dlp 的路径。不包含参数拼接细节（download-engine）与一般设置持久化（settings-preferences）。

## Task Routes

| Task | Read |
| --- | --- |
| 新增一种凭据 | `index.md` -> `api-contract.md` -> `backend-behavior.md` -> `frontend-behavior.md` -> `verification.md` |
| 改认证页 | `index.md` -> `frontend-behavior.md` -> `verification.md` |
| 排查"凭据不生效/保险库锁死" | `flow.md` -> `backend-behavior.md` -> `verification.md` |
| 安全审计 | `api-contract.md` -> `backend-behavior.md` -> `../rules/security.rules.md` |

## Related Domains

- `../download-engine/index.md`：`with_auth_args` 是唯一消费方。
- `../settings-preferences/index.md`：`auth.cookieFile`/`auth.cookieBrowser` 存在 `Config` 中。
- `../app-lifecycle/index.md`：启动时解锁时机与 macOS debug 差异。

## Historical References

- 无。

## Update Rules

- 新增密钥必须同步 `STRONGHOLD_KEYS`（前端）、`AuthSecrets`/`load_auth_secrets`（后端）与本文件表格。
- 任何把明文写入日志/配置文件/事件的改动都属于禁止项，见 `../rules/forbidden.rules.md`。
