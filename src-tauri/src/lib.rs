mod cards;
mod db;
mod db_hub;
mod debug_log;
mod event_sink;
mod events;
mod mtga_process;
mod parser;
mod router;
mod segmenter;
mod tailer;

use cards::{CardDatabase, CardInfo, SharedCards};
use db::{DeckSnapshotRecord, DeckWL, MatchRecord};
use db_hub::DbHub;
use event_sink::EventSink;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

#[derive(Debug, Serialize)]
struct SettingsSnapshot {
    /// Currently active user's MTGA user_id, or None if no user has been
    /// detected from the log yet (e.g. MTGA not running).
    player_id: Option<String>,
    /// Display name for the current user, pulled from a recent match where
    /// they appeared. None if no matches have been recorded yet.
    player_name: Option<String>,
    track_deck_history: bool,
    backup_on_launch: bool,
    developer_mode: bool,
}

#[tauri::command]
fn launch_mtga(path: Option<String>) -> Result<(), String> {
    mtga_process::launch(path.as_deref())
}

#[tauri::command]
fn get_mtga_status() -> bool {
    mtga_process::is_running()
}

#[tauri::command]
fn get_wl_stats(hub: State<Arc<Mutex<DbHub>>>) -> Result<Vec<DeckWL>, String> {
    let hub = hub.lock().map_err(|e| e.to_string())?;
    match hub.db() {
        Some(db) => db.get_wl_stats().map_err(|e| e.to_string()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
fn get_match_history(hub: State<Arc<Mutex<DbHub>>>) -> Result<Vec<MatchRecord>, String> {
    let hub = hub.lock().map_err(|e| e.to_string())?;
    match hub.db() {
        Some(db) => db.get_match_history(50).map_err(|e| e.to_string()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
fn get_decks(hub: State<Arc<Mutex<DbHub>>>) -> Result<Vec<DeckSnapshotRecord>, String> {
    let hub = hub.lock().map_err(|e| e.to_string())?;
    match hub.db() {
        Some(db) => db.get_decks().map_err(|e| e.to_string()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
fn get_settings(hub: State<Arc<Mutex<DbHub>>>) -> Result<SettingsSnapshot, String> {
    let hub = hub.lock().map_err(|e| e.to_string())?;
    let user_id = hub.current_user_id().map(|s| s.to_string());

    let (player_name, track_deck_history, backup_on_launch, developer_mode) = match hub.db() {
        Some(db) => {
            // Player name from the most recent match this user appears in
            let recent = db.get_recent_players(20).unwrap_or_default();
            let name = user_id
                .as_ref()
                .and_then(|uid| recent.iter().find(|(u, _)| u == uid).map(|(_, n)| n.clone()));
            let track = db
                .get_setting("track_deck_history")
                .map(|v| v == "true")
                .unwrap_or(true);
            let backup = db
                .get_setting("backup_on_launch")
                .map(|v| v == "true")
                .unwrap_or(true);
            let dev = db
                .get_setting("developer_mode")
                .map(|v| v == "true")
                .unwrap_or(false);
            (name, track, backup, dev)
        }
        None => (None, true, true, false),
    };

    Ok(SettingsSnapshot {
        player_id: user_id,
        player_name,
        track_deck_history,
        backup_on_launch,
        developer_mode,
    })
}

#[tauri::command]
fn set_app_setting(
    hub: State<Arc<Mutex<DbHub>>>,
    key: String,
    value: String,
) -> Result<(), String> {
    // Whitelist keys that are safe to set from the UI.
    let allowed = ["track_deck_history", "backup_on_launch", "developer_mode"];
    if !allowed.contains(&key.as_str()) {
        return Err(format!("setting '{}' is not user-configurable", key));
    }
    let hub = hub.lock().map_err(|e| e.to_string())?;
    let db = hub
        .db()
        .ok_or_else(|| "no active user — wait for MTGA to log in".to_string())?;
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
fn reset_stats(hub: State<Arc<Mutex<DbHub>>>) -> Result<(), String> {
    let hub = hub.lock().map_err(|e| e.to_string())?;
    let db = hub
        .db()
        .ok_or_else(|| "no active user — wait for MTGA to log in".to_string())?;
    db.reset_stats().map_err(|e| e.to_string())
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

/// Export full match history as a pretty-printed JSON file.
/// Shows a native Save As dialog so the user picks the location.
/// Returns the chosen path on success, or None if the user cancelled.
#[tauri::command]
fn export_match_history(
    app: tauri::AppHandle,
    hub: State<Arc<Mutex<DbHub>>>,
) -> Result<Option<String>, String> {
    let records = {
        let hub = hub.lock().map_err(|e| e.to_string())?;
        match hub.db() {
            Some(db) => db.get_match_history(u32::MAX).map_err(|e| e.to_string())?,
            None => vec![],
        }
    };
    let json = serde_json::to_string_pretty(&records).map_err(|e| e.to_string())?;

    use tauri_plugin_dialog::DialogExt;
    let path = app
        .dialog()
        .file()
        .set_file_name("mtga-match-history.json")
        .add_filter("JSON", &["json"])
        .blocking_save_file();

    match path {
        None => Ok(None), // user cancelled
        Some(file_path) => {
            let path_buf = match file_path {
                tauri_plugin_dialog::FilePath::Path(p) => p,
                tauri_plugin_dialog::FilePath::Url(url) => url
                    .to_file_path()
                    .map_err(|_| "invalid file URL from dialog".to_string())?,
            };
            std::fs::write(&path_buf, json).map_err(|e| e.to_string())?;
            Ok(Some(path_buf.to_string_lossy().to_string()))
        }
    }
}

/// Copy Player.log and debug.log to a user-chosen folder.
/// Only callable when developer_mode is enabled in settings.
#[tauri::command]
fn copy_logs_for_review(
    app: tauri::AppHandle,
    hub: State<Arc<Mutex<DbHub>>>,
) -> Result<String, String> {
    {
        let hub = hub.lock().map_err(|e| e.to_string())?;
        let enabled = hub
            .db()
            .and_then(|db| db.get_setting("developer_mode"))
            .map(|v| v == "true")
            .unwrap_or(false);
        if !enabled {
            return Err("developer mode is not enabled".to_string());
        }
    }

    use tauri_plugin_dialog::DialogExt;
    let dest_dir = match app.dialog().file().blocking_pick_folder() {
        None => return Err("cancelled".to_string()),
        Some(tauri_plugin_dialog::FilePath::Path(p)) => p,
        Some(tauri_plugin_dialog::FilePath::Url(url)) => url
            .to_file_path()
            .map_err(|_| "invalid folder URL from dialog".to_string())?,
    };

    let pairs = [
        (default_log_path(), "Player.log"),
        (default_debug_log_path(), "debug.log"),
    ];
    let mut copied = vec![];
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
    #[cfg(target_os = "windows")]
    {
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
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join("Wizards Of The Coast")
            .join("MTGA")
            .join("Player.log")
    }
}

/// Root data directory for the app. Per-user DBs live below this at `users/<user_id>/stats.db`.
fn default_app_root() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA").unwrap_or_default();
        std::path::Path::new(&app_data).join("local-client-mtga-stat-assistant")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::Path::new(&home)
            .join("Library")
            .join("Application Support")
            .join("local-client-mtga-stat-assistant")
    }
}

fn watch_log(app_handle: tauri::AppHandle, hub: Arc<Mutex<DbHub>>, cards: SharedCards) {
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
            // Write to DB and collect any synthesized follow-up events
            let synthesized: Vec<events::GameEvent> = {
                if let (Ok(mut hub), Ok(card_db)) = (hub.lock(), cards.read()) {
                    sink.process(&event, &mut hub, &card_db)
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
    default_app_root().join("debug.log")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug_log::init(default_debug_log_path());
    dlog!("[startup] app starting BUILD-2026-04-27 (per-user DB hot-swap)");

    let app_root = default_app_root();
    dlog!("[startup] app root: {:?}", app_root);

    // Open the most-recently-used per-user DB at startup so the UI has
    // something to show before MTGA logs in. The router will swap to the
    // correct user as soon as a header line is observed.
    let mut hub = DbHub::new(app_root.clone());
    if let Some(uid) = db_hub::find_most_recent_user(&app_root) {
        dlog!("[startup] preselecting most-recent user: {}", uid);
        if let Err(e) = hub.switch(&uid) {
            dlog!("[startup] preselect failed: {}", e);
        }
    } else {
        dlog!("[startup] no per-user DBs yet — waiting for first MTGA login");
    }
    let hub = Arc::new(Mutex::new(hub));

    let card_db: SharedCards = Arc::new(RwLock::new(CardDatabase::load()));
    cards::spawn_watcher(card_db.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(hub.clone())
        .manage(card_db.clone())
        .setup(move |app| {
            let hub = app.state::<Arc<Mutex<DbHub>>>().inner().clone();
            let cards = app.state::<SharedCards>().inner().clone();
            watch_log(app.handle().clone(), hub, cards);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launch_mtga,
            get_mtga_status,
            get_wl_stats,
            get_match_history,
            get_decks,
            get_card_info,
            get_settings,
            set_app_setting,
            copy_logs_for_review,
            reset_stats,
            export_match_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
