mod cards;
mod db;
mod debug_log;
mod event_sink;
mod events;
mod mtga_process;
mod parser;
mod router;
mod segmenter;
mod tailer;

use cards::{CardDatabase, CardInfo, SharedCards};
use db::{Db, DeckWL, MatchRecord};
use event_sink::EventSink;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, mpsc, Arc, Mutex, RwLock};
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

#[tauri::command]
fn get_card_info(
    grp_ids: Vec<u32>,
    cards: State<SharedCards>,
) -> HashMap<u32, CardInfo> {
    cards
        .read()
        .map(|c| c.get_info(&grp_ids))
        .unwrap_or_default()
}

/// Snapshot Player.log and debug.log into the parent folder for inspection.
/// Hardcoded destination is the working directory I (Claude) can read while
/// developing — saves the user from manually copying files between sessions.
#[tauri::command]
fn copy_logs_for_review() -> Result<String, String> {
    let dest_dir = std::path::PathBuf::from("C:/Users/renga/Claude/MTGA");
    if !dest_dir.exists() {
        return Err(format!("destination not found: {:?}", dest_dir));
    }
    let pairs = [
        (default_log_path(), "Player.log"),
        (default_debug_log_path(), "debug.log"),
    ];
    let mut copied: Vec<String> = vec![];
    for (src, name) in &pairs {
        let dst = dest_dir.join(name);
        match std::fs::copy(src, &dst) {
            Ok(bytes) => copied.push(format!("{} ({} bytes)", name, bytes)),
            Err(e) => return Err(format!("copy {}: {}", name, e)),
        }
    }
    Ok(copied.join(", "))
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

fn watch_log(app_handle: tauri::AppHandle, db: Arc<Mutex<Db>>, cards: SharedCards) {
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

        let mut sink = {
            let guard = db.lock().expect("db lock for sink init");
            EventSink::new(&guard)
        };

        for event in event_rx {
            dlog!("[event] {:?}", event);
            // Write to DB and collect any synthesized follow-up events
            let synthesized: Vec<events::GameEvent> = {
                if let (Ok(mut db), Ok(card_db)) = (db.lock(), cards.read()) {
                    sink.process(&event, &mut db, &card_db)
                } else {
                    vec![]
                }
            };
            // Push original event, then synthesized ones — the frontend's
            // reducer relies on PlayerIdentified arriving right after MatchStarted
            let _ = app_handle.emit("game_event", &event);
            for extra in synthesized {
                dlog!("[event] (synthesized) {:?}", extra);
                let _ = app_handle.emit("game_event", &extra);
            }
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
    dlog!("[startup] app starting BUILD-2026-04-26-c (logs why GRE chunks emit zero events)");

    let db_path = default_db_path();
    dlog!("[startup] db path: {:?}", db_path);
    let db = Db::open(&db_path).expect("failed to open database");
    let db = Arc::new(Mutex::new(db));

    let card_db: SharedCards = Arc::new(RwLock::new(CardDatabase::load()));
    cards::spawn_watcher(card_db.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(db.clone())
        .manage(card_db.clone())
        .setup(move |app| {
            let db = app.state::<Arc<Mutex<Db>>>().inner().clone();
            let cards = app.state::<SharedCards>().inner().clone();
            watch_log(app.handle().clone(), db, cards);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launch_mtga,
            get_mtga_status,
            get_wl_stats,
            get_match_history,
            get_card_info,
            copy_logs_for_review
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
