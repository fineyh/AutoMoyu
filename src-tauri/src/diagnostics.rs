//! 导出诊断包：日志、校准结果与截图、设置、状态快照、系统信息打成一个 zip。

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use zip::write::SimpleFileOptions;

use crate::settings::data_dir;

fn add_dir(zip: &mut zip::ZipWriter<std::fs::File>, dir: &Path, prefix: &str, opts: SimpleFileOptions) -> Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Ok(()) };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() {
            zip.start_file(format!("{prefix}/{}", e.file_name().to_string_lossy()), opts)?;
            zip.write_all(&std::fs::read(&p)?)?;
        }
    }
    Ok(())
}

pub fn export(out: &Path, status_json: &str) -> Result<()> {
    let f = std::fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let base = data_dir();
    add_dir(&mut zip, &base.join("logs"), "logs", opts)?;
    add_dir(&mut zip, &base.join("calibration"), "calibration", opts)?;
    if let Ok(s) = std::fs::read(base.join("settings.json")) {
        zip.start_file("settings.json", opts)?;
        zip.write_all(&s)?;
    }
    zip.start_file("status.json", opts)?;
    zip.write_all(status_json.as_bytes())?;
    // 诊断包会被贴到公开 Issue 里：只保留像 Minecraft 的窗口标题，其他窗口（浏览器标签、聊天）只留进程名和尺寸
    let windows: Vec<String> = moyu_win::window::list_windows()
        .iter()
        .map(|w| {
            let title = if w.is_minecraft() || w.title.to_lowercase().contains("minecraft") { w.title.as_str() } else { "(标题已隐藏)" };
            format!("  {} · {} · {}x{}{}", w.process, title, w.w, w.h, if w.minimized { " (最小化)" } else { "" })
        })
        .collect();
    let sys = format!(
        "AutoMoyu {}\nOS: {} {}\n时间: {}\n可见窗口:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        os_version(),
        chrono::Local::now().to_rfc3339(),
        windows.join("\n")
    );
    zip.start_file("system.txt", opts)?;
    zip.write_all(sys.as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn os_version() -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/c", "ver"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
