---
status: current
layer: eval
domain: evals
canonical_for:
  - release-checklist
related:
  - docs/current/platform/release-and-distribution.md
last_verified: 2026-09-17
---

# 发布检查清单

## 版本与元数据

- [ ] `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三处版本号一致。
- [ ] `README.md` 的下载表与产物命名规则仍与实际一致。
- [ ] locale 文件（前端 + 后端）键一致，无缺失导致回退的界面文案。
- [ ] `npm run licenses` 成功，产物许可证文件齐全。

## 代码冻结检查

- [ ] `main` 上 CI 全绿（vue-ci：lint/unit/e2e/build；rust-ci：clippy/fmt/test）。
- [ ] 无未闭环的高优先级 change（worklog 中不存在长期 `pending review` 的循环）。
- [ ] 无遗留 `TODO`/调试代码进入本次发布（按 `rules/forbidden.rules.md` 检查）。
- [ ] 诊断规则文件与前端 i18n 的 `errors.runner.*` 一一对应。

## 构建与签名

- [ ] `release` 分支已合并 main 的最新内容。
- [ ] `publish.yml` 6 个目标全部成功；Release 附件齐全（nsis/dmg/AppImage/deb/rpm 与 updater 产物）。
- [ ] Windows 代码签名与 macOS 签名/公证生效（安装时无未知发布者警告）。
- [ ] `latest.json`（更新源）可访问且版本号正确。

## 二进制清单

- [ ] `pages.yml` 在 release 分支成功：`manifest:gen` → `sign` → `verify` 全过。
- [ ] `docs/manifest/manifest.json` 的 `generatedAt` 为本次时间；`manifest.sig` 更新。
- [ ] 清单中的工具版本与平台覆盖符合预期（`yt-dlp`、`ffmpeg`，各平台键齐全）。
- [ ] CI 使用的 `ED25519_PUB_KEY_HEX` 与 `binaries_manager.rs` 的 `MANIFEST_PUB_KEY` 一致。
- [ ] 在一台干净机器上验证：启动 → 安装页 → 安装成功 → 下载成功。

## 分发渠道

- [ ] `distribute.yml` 已执行：winget 清单提交、Snap 从 edge 提升到 stable。
- [ ] Microsoft Store（`msix.yml`）产物提交与状态确认。
- [ ] 官网（GitHub Pages）已部署，下载链接可用。

## 三平台冒烟

| 平台 | 检查项 |
| --- | --- |
| Windows | 安装/卸载、托盘、关闭行为（exit/hide）、全局快捷键（Alt+Shift）、任务栏进度与徽标、更新安装 |
| macOS (x64/arm64) | dmg 安装、关闭仅隐藏、Ctrl+Shift 快捷键、程序坞徽标、通知权限、钥匙串访问 |
| Linux (deb/rpm/AppImage) | AppImage 可执行、通知（notify_rust）、托盘（视桌面环境）、自启动项、glib/webkit 依赖齐全 |

通用：首次启动无 yt-dlp → 进入安装页；下载一个视频并确认文件可播放；暂停/恢复；语言切换；设置重启保持。

## 发布后

- [ ] Release 说明包含变更摘要与已知问题。
- [ ] 监控 Sentry 24 小时（错误量级与新增错误类型）。
- [ ] 若发现回归：优先用 `hotfix:` 分支处理，按 `docs/evals/bugfix-checklist.md` 走流程。
- [ ] 复盘（如有事故）：写 `docs/postmortems/` 并指向护栏。

## Verification

```bash
# 本地预检
npm run lint:fix && npm run test:unit && npm run test:e2e && npm run build
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test
npm run manifest:gen && npm run manifest:sign && npm run manifest:verify   # 需签名密钥
python docs/scripts/check_doc_runtime.py docs
```
