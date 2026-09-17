---
status: current
layer: domain
domain: toolchain
canonical_for:
  - toolchain-verification
related:
  - docs/evals/release-checklist.md
last_verified: 2026-09-17
---

# toolchain 验证

## Purpose

给出二进制安装链路的验证方式：本地如何模拟、CI 如何覆盖、以及必须保持的安全语义。

## Current Behavior

### 自动化覆盖

| 关注点 | 方式 |
| --- | --- |
| 安装页与进度 UI | `tests/e2e/binaries.spec.ts`（mock `binaries_check`/`binaries_ensure`，见 `tests/utils/mocks/binaryHandlers.ts`） |
| 提取器 | `binaries_extractor.rs` 内置单测（zip/tar.bz2、entry 缺失、多文件歧义、目录穿越） |
| 清单生成/签名/校验 | `npm run manifest:gen` → `npm run manifest:sign` → `npm run manifest:verify`（CI 在 release 分支执行） |
| 命令返回结构 | E2E mock 与 `CheckResult` 的字段一一对应；Rust 侧无独立单测 |

### 本地生成并校验清单

```bash
npm run manifest:fetch          # 刷新 sources.ts 中的版本/产物（需要网络，可用 GITHUB_TOKEN 提升额度）
npm run manifest:gen            # 生成 docs/manifest/manifest.json（含 sha256）
npm run manifest:sign           # 需要 ED25519_PRIV_KEY_B64；输出 manifest.sig
npm run manifest:verify         # 需要 ED25519_PUB_KEY_HEX；校验签名
```

`docs/manifest/` 已被 `.gitignore` 忽略，仅作为 Pages 构建产物存在。

### 手工验收步骤

1. **首次安装**：删除 `bin_dir` 内容与 `metadata.json` → 启动应用 → 出现 `/install`；yt-dlp 与 ffmpeg 各自显示进度；完成后 5 秒倒计时进入主页。
2. **跳过检查**：设置 → 关于/更新 中关闭"更新二进制" → 再次删除 `bin_dir` → 启动不再跳转安装页（此时下载会失败，符合预期）。
3. **校验失败**：手工把 `manifest.json` 中某工具的 `sha256` 改错（本地起一个替换清单源）→ 安装失败并显示 `[download_verify] sha256 mismatch`。
4. **签名失败**：把 `manifest.sig` 换成随机字节 → `binaries_check` 返回错误，不出现安装页（需观察日志/控制台）。
5. **锁定**：把 `bin_dir/metadata.json` 的 `is_locked` 改为 `true` → `check` 返回空数组，即使版本过期也不安装。
6. **版本升级**：手动把 `metadata.json` 的版本改成旧值 → 重启 → 触发重装（覆盖已有文件，验证 hoist 覆盖行为）。
7. **真实下载**：安装完成后执行一次下载，确认 `PATH` 前置生效（日志中 `has_*` 摘要正常、yt-dlp 能调用到 ffmpeg）。

### 必须保持的既有语义（Must-Not-Break）

- 清单签名校验必须先于任何安装动作；不得为"容错"而容忍签名失败。
- 归档 sha256 不匹配必须放弃安装（不留 canonical 文件）。
- 解压必须拒绝绝对路径与 `..` 条目。
- `metadata.json` 只在工具完全成功时写入该工具的版本。
- 平台键必须支持"精确优先、OS 前缀兜底"的匹配（否则新架构平台会全量失败）。

## Boundaries

- 不覆盖真实 GitHub 下载的稳定性与速率；不覆盖代理场景（二进制下载不使用应用代理设置）。
- 不在 CI 中执行真实安装（需要网络与写权限）。

## Contracts

- 清单结构：`docs/current/domains/toolchain/api-contract.md`。
- 发布流水线：`docs/current/platform/release-and-distribution.md`。
- 发布前检查清单：`docs/evals/release-checklist.md`。

## Failure And Edge Cases

- 若 CI 的 `ED25519_PUB_KEY_HEX` 与应用内硬编码公钥不一致，签名校验必然失败——这是"静默换钥匙"事故的典型来源，发布前必须核对两处。
- `sources.ts` 中若把工具版本写成"latest"之类的可变值，`metadata.json` 比较会永远不等，导致每次启动都重装。
- 提取器遇到新压缩格式（如 `.tar.gz`、`.7z`）会走到 `UnsupportedEntry`/扩展名不匹配分支，需要同步扩展 `extract_*` 与 `download_and_verify` 的分支。

## Verification

- 全量：`cd src-tauri && cargo test`（含提取器用例）；`npm run test:e2e -- binaries`；`npm run manifest:verify`（本地配置好公钥时）。
- 回归矩阵：`docs/evals/regression-matrix.md` 中 toolchain 相关行。
