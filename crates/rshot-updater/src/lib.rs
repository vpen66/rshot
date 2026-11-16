//! GitHub Releases 自动更新：检查最新版本、下载并替换当前可执行文件。
//!
//! 所有函数都是阻塞式的（内部走 reqwest blocking），必须在后台线程调用，
//! 不能直接在托盘事件循环线程里跑。

use anyhow::{Context, Result, bail};
use self_update::Status;
use self_update::backends::github::Update;

pub const REPO_OWNER: &str = "vpen66";
pub const REPO_NAME: &str = "rshot";
const BIN_NAME: &str = "rshot";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub date: String,
}

#[derive(Debug, Clone)]
pub enum CheckOutcome {
    UpToDate,
    Available(UpdateInfo),
}

/// 检查最新 Release。仓库还没有任何 Release（404）时视为已是最新。
pub fn check(current_version: &str) -> Result<CheckOutcome> {
    let updater = Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .current_version(current_version)
        .show_output(false)
        .no_confirm(true)
        .build()
        .context("初始化更新器失败")?;

    let release = match updater.get_latest_release() {
        Ok(r) => r,
        Err(e) if e.to_string().contains("404") => return Ok(CheckOutcome::UpToDate),
        Err(e) => return Err(anyhow::Error::new(e).context("无法获取 GitHub Release 信息")),
    };

    if !self_update::version::bump_is_greater(current_version, &release.version)? {
        return Ok(CheckOutcome::UpToDate);
    }
    if !release.has_target_asset(self_update::get_target()) {
        bail!(
            "最新版本 v{} 没有提供当前平台（{}）的安装包",
            release.version,
            self_update::get_target()
        );
    }
    Ok(CheckOutcome::Available(UpdateInfo {
        version: release.version,
        notes: release.body.unwrap_or_default(),
        date: release.date,
    }))
}

/// 下载并安装指定版本，替换当前可执行文件。成功后调用方负责重启进程。
///
/// 显式指定目标 tag：self_update 默认的"兼容版本"过滤会跳过 0.x 阶段的
/// minor 升级（0.1 -> 0.2 被视为不兼容），指定 tag 可绕过该过滤。
pub fn install(current_version: &str, version: &str) -> Result<()> {
    let updater = Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .current_version(current_version)
        .target_version_tag(&format!("v{version}"))
        .show_output(false)
        .no_confirm(true)
        .build()
        .context("初始化更新器失败")?;

    match updater.update().context("下载或安装更新失败")? {
        Status::Updated(v) => {
            log::info!("Updated rshot to v{v}");
            Ok(())
        }
        Status::UpToDate(_) => bail!("目标版本 v{version} 不高于当前版本，未执行更新"),
    }
}
