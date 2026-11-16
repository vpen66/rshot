# AGENTS.md

## 本机运行/验证速查（ide-build-run 维护）
- 栈 / 包管理器：Rust workspace（edition 2024，需 Rust >= 1.85）/ cargo
- 检查：`cargo check --workspace`
- 严格：`cargo clippy --workspace --all-targets`（存量 warning 未清零，勿加 `-D warnings`）；`cargo fmt --all --check` 存在历史欠账，只对本轮改动包跑 `cargo fmt -p <pkg>`
- 测试：`cargo test --workspace`（headless 安全）
- 发布构建：`cargo build --release --package rshot`
- 启动方式：`cargo run`（常驻菜单栏）；`--capture` 区域截图 / `--fullscreen` 全屏截图
- 端口：无
- 就绪标志：日志 "RShot is resident in the menu bar"
- 其他前置：macOS 需屏幕录制权限（TCC）；旧的菜单栏进程会一直驻留，重新构建后需提醒用户手动退出并重启，不要代为 kill

## 发布与自动更新
- GitHub 仓库：vpen66/rshot；Release 由 `.github/workflows/release.yml` 在 `v*` tag 触发
- 更新检查走 GitHub Releases API（crates/rshot-updater，self_update + rustls）
- 发布资产命名必须含 target triple（如 `rshot-v0.2.0-aarch64-apple-darwin.tar.gz`），二进制位于压缩包根目录，否则 self_update 匹配/解压失败
- 版本号在 tag 触发时同步进 workspace 全部 Cargo.toml；本地开发版本以根 Cargo.toml 为准