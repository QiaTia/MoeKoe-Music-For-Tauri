//! 拉起安装包与 macOS Gatekeeper 辅助。

use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// 打开安装包。
///
/// Linux 的 AppImage 是自包含可执行文件：先补 `0755` 再**直接执行**——部分桌面
/// 环境会把它当文档打开（丢给解压器/编辑器），用户看到的是「什么都没有发生」。
/// 其余格式（exe / msi / dmg / deb / rpm）交给系统关联程序处理。
pub(super) fn launch_installer(app: &AppHandle, path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let is_appimage = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("appimage"))
            .unwrap_or(false);
        if is_appimage {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(path).map_err(|e| format!("读取安装包失败: {e}"))?;
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)
                .map_err(|e| format!("设置可执行权限失败: {e}"))?;
            std::process::Command::new(path)
                .spawn()
                .map_err(|e| format!("启动 AppImage 失败: {e}"))?;
            return Ok(());
        }
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("无法打开安装包: {e}"))
}

/// 从可执行文件向上回溯，找到所属的 `.app` bundle。
///
/// 找不到必须返回 `None`。早期实现用 `loop` 一路回溯到根目录，`tauri dev` 或
/// 未打包运行时会把它当成 bundle，生成 `sudo xattr -cr '/'` —— 用户照着粘贴
/// 等于递归清除整盘的隔离属性。
///
/// 仅 macOS 分支与单测需要它，其它平台的正式构建不编译，避免 unused 警告。
#[cfg(any(target_os = "macos", test))]
pub(super) fn find_app_bundle(exe: &Path) -> Option<&Path> {
    exe.ancestors()
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("app"))
}

/// macOS Gatekeeper 放行脚本：清除 .app 的隔离属性。
///
/// 未签名/未公证的应用经自动更新替换后，首次打开可能被拦截（「已损坏」）。
/// 路径从当前可执行文件回溯到 `.app` bundle，安装到非默认位置也准确。
/// 非 macOS、或当前不是 `.app` 形态时返回空串（前端按空值不展示）。
///
/// 命令包装在 `updater::update_gatekeeper_script`：`#[tauri::command]` 宏生成的
/// `__cmd__*` 辅助项无法跨模块 `pub use` 重导出，因此命令必须定义在使用处。
pub(super) fn gatekeeper_script() -> String {
    #[cfg(target_os = "macos")]
    {
        let Ok(exe) = std::env::current_exe() else {
            return String::new();
        };
        let Some(bundle) = find_app_bundle(&exe) else {
            eprintln!(
                "[updater] 未找到 .app bundle，跳过 Gatekeeper 建议: {}",
                exe.display()
            );
            return String::new();
        };
        // 该命令由用户手动粘贴执行：路径要按 shell 字面量处理，单引号内不做任何展开
        let quoted = format!("'{}'", bundle.to_string_lossy().replace('\'', "'\\''"));
        format!("sudo xattr -cr {quoted}")
    }
    #[cfg(not(target_os = "macos"))]
    {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_app_bundle_requires_dot_app() {
        // 正常打包形态：向上回溯到 .app
        let exe = Path::new("/Applications/MoeKoe Music.app/Contents/MacOS/moekoe-music");
        assert_eq!(
            find_app_bundle(exe),
            Some(Path::new("/Applications/MoeKoe Music.app"))
        );
        // 开发运行 / 裸二进制：必须返回 None，绝不能回落到根目录
        // （否则生成的命令是 `sudo xattr -cr '/'`）
        assert_eq!(find_app_bundle(Path::new("/usr/local/bin/moekoe-music")), None);
        assert_eq!(find_app_bundle(Path::new("/moekoe-music")), None);
    }
}
