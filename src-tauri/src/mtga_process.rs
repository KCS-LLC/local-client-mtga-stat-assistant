use sysinfo::{ProcessesToUpdate, System};

#[cfg(target_os = "windows")]
const MTGA_PROCESS: &str = "MTGA.exe";
#[cfg(target_os = "macos")]
const MTGA_PROCESS: &str = "MTGA";

pub fn is_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    sys.processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case(MTGA_PROCESS))
}

pub fn launch(path: Option<&str>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let exe = path.unwrap_or("C:\\Program Files\\Wizards of the Coast\\MTGA\\MTGA.exe");
        std::process::Command::new(exe)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch MTGA: {}", e))
    }
    #[cfg(target_os = "macos")]
    {
        // On macOS use `open -a` to launch the app bundle; path is the app name or full .app path
        let app = path.unwrap_or("MTG Arena");
        std::process::Command::new("open")
            .arg("-a")
            .arg(app)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch MTGA: {}", e))
    }
}
