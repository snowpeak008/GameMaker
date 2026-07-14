#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use adm_new_root_launcher::{
    LaunchLayout, clear_error_report, launch_product, webview2_runtime_available,
    write_error_report,
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const CHECK_ARGUMENT: &str = "--check-launcher";

fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let check_only = arguments
        .iter()
        .any(|argument| argument == OsStr::new(CHECK_ARGUMENT));
    let launcher_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let source_root = launcher_path.parent().unwrap_or_else(|| Path::new("."));

    match run(&launcher_path, &arguments, check_only) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            if !check_only && let Ok(report_path) = write_error_report(source_root, &message) {
                let _ = Command::new("notepad.exe").arg(report_path).spawn();
            }
            ExitCode::from(2)
        }
    }
}

fn run(launcher_path: &Path, arguments: &[OsString], check_only: bool) -> Result<(), String> {
    let layout = LaunchLayout::from_launcher_path(launcher_path)?;
    layout.validate()?;
    if !webview2_runtime_available() {
        return Err(
            "Microsoft Edge WebView2 Runtime was not detected. Install the Evergreen Runtime and try again."
                .to_string(),
        );
    }
    layout.prepare_data_root()?;
    if check_only {
        return Ok(());
    }

    clear_error_report(&layout.source_root);
    let child_arguments = arguments
        .iter()
        .filter(|argument| argument.as_os_str() != OsStr::new(CHECK_ARGUMENT))
        .cloned()
        .collect::<Vec<_>>();
    let _child = launch_product(&layout, &child_arguments)?;
    Ok(())
}
