mod mtga_process;
mod segmenter;
mod tailer;

use std::sync::{atomic::AtomicBool, mpsc, Arc};
use std::thread;
use std::time::Duration;
use tauri::Emitter;

#[tauri::command]
fn launch_mtga(path: Option<String>) -> Result<(), String> {
    mtga_process::launch(path.as_deref())
}

#[tauri::command]
fn get_mtga_status() -> bool {
    mtga_process::is_running()
}

fn default_log_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_default();
    let local_low = std::path::Path::new(&app_data)
        .parent()
        .map(|p| p.join("LocalLow"))
        .unwrap_or_else(|| std::path::PathBuf::from("LocalLow"));
    local_low
        .join("Wizards Of The Coast")
        .join("MTGA")
        .join("Player.log")
}

fn watch_log(app_handle: tauri::AppHandle) {
    thread::spawn(move || {
        let log_path = default_log_path();
        let mtga_was_running = mtga_process::is_running();

        let _ = app_handle.emit("mtga_status", mtga_was_running);

        if !mtga_was_running {
            // Wait for the user to launch MTGA (via button or manually)
            loop {
                thread::sleep(Duration::from_secs(1));
                if mtga_process::is_running() {
                    let _ = app_handle.emit("mtga_status", true);
                    // Give MTGA a moment to create/clear the log file
                    while !log_path.exists() {
                        thread::sleep(Duration::from_millis(500));
                    }
                    break;
                }
            }
        }

        // If MTGA was already running we missed the session start — begin at
        // end of file and pick up from the next match forward.
        let start_pos = if mtga_was_running {
            tailer::StartPosition::End
        } else {
            tailer::StartPosition::Beginning
        };

        let (line_tx, line_rx) = mpsc::channel::<String>();
        let (chunk_tx, chunk_rx) = mpsc::channel::<segmenter::Chunk>();
        let running = Arc::new(AtomicBool::new(true));

        tailer::start(log_path, start_pos, line_tx, running.clone());
        segmenter::start(line_rx, chunk_tx);

        // Relay chunks to the frontend for now.
        // This will be replaced by the router in the next step.
        for chunk in chunk_rx {
            let _ = app_handle.emit("log_chunk", &chunk.content);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            watch_log(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![launch_mtga, get_mtga_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
