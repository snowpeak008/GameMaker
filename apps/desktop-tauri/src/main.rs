#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--smoke") {
        let executable_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let report = desktop_tauri::release_smoke_report_for(&executable_dir);
        let encoded = serde_json::to_string_pretty(&report)
            .unwrap_or_else(|_| "{\"status\":\"blocked\"}".to_string());
        if let Some(index) = args.iter().position(|arg| arg == "--smoke-report")
            && let Some(path) = args.get(index + 1)
        {
            let _ = std::fs::write(path, format!("{encoded}\n"));
        }
        println!("{encoded}");
        std::process::exit(if report.status == "passed" { 0 } else { 1 });
    }
    desktop_tauri::run();
}
