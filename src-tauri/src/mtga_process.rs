use sysinfo::{ProcessesToUpdate, System};

const MTGA_PROCESS: &str = "MTGA.exe";
const MTGA_DEFAULT_PATH: &str = "C:\\Program Files\\Wizards of the Coast\\MTGA\\MTGA.exe";

pub fn is_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    sys.processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case(MTGA_PROCESS))
}

pub fn launch(path: Option<&str>) -> Result<(), String> {
    let exe = path.unwrap_or(MTGA_DEFAULT_PATH);
    std::process::Command::new(exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch MTGA: {}", e))
}
