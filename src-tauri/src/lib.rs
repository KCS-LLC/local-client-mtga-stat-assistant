mod db;
mod debug_log;
mod event_sink;
mod events;
mod mtga_process;
mod parser;
mod router;
mod segmenter;
mod tailer;

use db::{Db, DeckWL, MatchRecord};
use event_sink::EventSink;
use std::sync::{atomic::AtomicBool, mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

#[tauri::command]
fn launch_mtga(path: Option<String>) -> Result<(), String> {
    mtga_process::launch(path.as_deref())
}

#[tauri::command]
fn get_mtga_status() -> bool {
    mtga_process::is_running()
}

#[tauri::command]
fn get_wl_stats(db: State<Arc<Mutex<Db>>>) -> Result<Vec<DeckWL>, String> {
    db.lock()
        .map_err(|e| e.to_string())?
        .get_wl_stats()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_match_history(db: State<Arc<Mutex<Db>>>) -> Result<Vec<MatchRecord>, String> {
    db.lock()
        .map_err(|e| e.to_string())?
        .get_match_history(50)
        .map_err(|e| e.to_string())
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

fn default_db_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_default();
    std::path::Path::new(&app_data)
        .join("local-client-mtga-stat-assistant")
        .join("stats.db")
}

fn watch_log(app_handle: tauri::AppHandle, db: Arc<Mutex<Db>>) {
    thread::spawn(move || {
        let log_path = default_log_path();
        let mtga_was_running = mtga_process::is_running();

        dlog!("[startup] log path: {:?}", log_path);
        dlog!("[startup] log exists: {}", log_path.exists());
        dlog!("[startup] mtga running at startup: {}", mtga_was_running);

        let _ = app_handle.emit("mtga_status", mtga_was_running);

        if !mtga_was_running {
            loop {
                thread::sleep(Duration::from_secs(1));
                if mtga_process::is_running() {
                    dlog!("[startup] mtga process detected");
                    let _ = app_handle.emit("mtga_status", true);
                    while !log_path.exists() {
                        thread::sleep(Duration::from_millis(500));
                    }
                    dlog!("[startup] log file ready, starting tailer");
                    break;
                }
            }
        }

        let start_pos = if mtga_was_running {
            tailer::StartPosition::End
        } else {
            tailer::StartPosition::Beginning
        };
        dlog!(
            "[startup] tailer start position: {}",
            if mtga_was_running { "End" } else { "Beginning" }
        );

        let (line_tx, line_rx) = mpsc::channel::<String>();
        let (chunk_tx, chunk_rx) = mpsc::channel::<segmenter::Chunk>();
        let (event_tx, event_rx) = mpsc::channel::<events::GameEvent>();
        let running = Arc::new(AtomicBool::new(true));

        tailer::start(log_path, start_pos, line_tx, running.clone());
        segmenter::start(line_rx, chunk_tx);
        router::start(chunk_rx, event_tx);

        let mut sink = EventSink::new();

        for event in event_rx {
            dlog!("[event] {:?}", event);
            // Write to DB
            if let Ok(mut db) = db.lock() {
                sink.process(&event, &mut db);
            }
            // Push to frontend
            let _ = app_handle.emit("game_event", &event);
        }
    });
}

fn default_debug_log_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_default();
    std::path::Path::new(&app_data)
        .join("local-client-mtga-stat-assistant")
        .join("debug.log")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug_log::init(default_debug_log_path());
    dlog!("[startup] app starting");

    let db_path = default_db_path();
    dlog!("[startup] db path: {:?}", db_path);
    let db = Db::open(&db_path).expect("failed to open database");
    let db = Arc::new(Mutex::new(db));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(db.clone())
        .setup(|app| {
            let db = app.state::<Arc<Mutex<Db>>>().inner().clone();
            watch_log(app.handle().clone(), db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launch_mtga,
            get_mtga_status,
            get_wl_stats,
            get_match_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
