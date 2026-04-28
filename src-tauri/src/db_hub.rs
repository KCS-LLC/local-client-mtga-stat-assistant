use crate::db::Db;
use crate::dlog;
use std::path::{Path, PathBuf};

/// Owns the currently-active stats DB and the path layout for per-user
/// databases. `<root>/users/<user_id>/stats.db` is the canonical path for
/// each MTGA user's stats. The hub holds at most one open Db at a time —
/// the one that belongs to the user whose match data is currently arriving.
pub struct DbHub {
    root: PathBuf,
    current: Option<(String, Db)>,
}

impl DbHub {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            current: None,
        }
    }

    /// Switch to (or open) the database for the given user. Returns Ok(true)
    /// if a swap actually happened, Ok(false) if we were already on this
    /// user. Creates the user's folder on first use, and rescues a legacy
    /// single-tenant stats.db into the per-user folder if appropriate.
    pub fn switch(&mut self, user_id: &str) -> Result<bool, String> {
        if self
            .current
            .as_ref()
            .is_some_and(|(cur, _)| cur == user_id)
        {
            return Ok(false);
        }

        let db_path = self.user_db_path(user_id);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
        }

        // Try to rescue pre-rewrite stats.db sitting at the root. Only do
        // this when the per-user DB doesn't yet exist (we'd never overwrite
        // a real per-user DB) and the legacy file's stored player_id is
        // either empty or already matches this user (don't migrate someone
        // else's data into your folder).
        if !db_path.exists() {
            self.try_rescue_legacy(user_id, &db_path);
        }

        // Backup before opening for use, if the per-user setting allows.
        // We peek by opening read-only (this also runs migrations safely on
        // an existing DB; throwaway). Newly-created DBs skip backup since
        // there's nothing yet.
        if db_path.exists() {
            let should_backup = Db::open(&db_path)
                .ok()
                .and_then(|d| d.get_setting("backup_on_launch"))
                .map(|v| v == "true")
                .unwrap_or(true);
            if should_backup {
                let bak = db_path.with_extension("db.bak");
                match std::fs::copy(&db_path, &bak) {
                    Ok(bytes) => dlog!(
                        "[db_hub] backed up {} → {} ({} bytes)",
                        db_path.display(),
                        bak.display(),
                        bytes
                    ),
                    Err(e) => dlog!("[db_hub] backup failed: {}", e),
                }
            }
        }

        let db = Db::open(&db_path).map_err(|e| format!("open: {}", e))?;
        dlog!(
            "[db_hub] switched to user {} (db {})",
            user_id,
            db_path.display()
        );
        self.current = Some((user_id.to_string(), db));
        Ok(true)
    }

    pub fn current_user_id(&self) -> Option<&str> {
        self.current.as_ref().map(|(uid, _)| uid.as_str())
    }

    pub fn db(&self) -> Option<&Db> {
        self.current.as_ref().map(|(_, db)| db)
    }

    pub fn db_mut(&mut self) -> Option<&mut Db> {
        self.current.as_mut().map(|(_, db)| db)
    }

    fn user_db_path(&self, user_id: &str) -> PathBuf {
        self.root.join("users").join(user_id).join("stats.db")
    }

    /// Copy the legacy single-tenant stats.db into the per-user path when it
    /// belongs to (or could plausibly belong to) this user. After a
    /// successful copy, the legacy file is renamed to stats.db.legacy so the
    /// rescue doesn't run again on subsequent user switches.
    fn try_rescue_legacy(&self, user_id: &str, dest: &Path) {
        let legacy_path = self.root.join("stats.db");
        if !legacy_path.exists() {
            return;
        }

        let legacy_player_id = Db::open(&legacy_path)
            .ok()
            .and_then(|d| d.get_setting("player_id"));

        let safe_to_rescue = match legacy_player_id.as_deref() {
            Some(stored) if stored == user_id => true,
            Some(_) => {
                dlog!(
                    "[db_hub] legacy DB belongs to a different player ({:?}); not rescuing for {}",
                    legacy_player_id,
                    user_id
                );
                false
            }
            None => true, // unset; fair to assume the only previous user
        };
        if !safe_to_rescue {
            return;
        }

        match std::fs::copy(&legacy_path, dest) {
            Ok(bytes) => {
                dlog!(
                    "[db_hub] rescued legacy DB: {} → {} ({} bytes)",
                    legacy_path.display(),
                    dest.display(),
                    bytes
                );
                // Mark legacy file so we don't rescue the same data twice if
                // a different user logs in later.
                let marked = legacy_path.with_extension("db.legacy");
                if let Err(e) = std::fs::rename(&legacy_path, &marked) {
                    dlog!("[db_hub] could not rename legacy file: {}", e);
                }
            }
            Err(e) => dlog!("[db_hub] legacy rescue copy failed: {}", e),
        }
    }
}

/// Find the most-recently-modified per-user DB so we can preselect a sensible
/// default at startup (e.g., the user from the last session). Returns the
/// user_id, or None if the users directory is empty / missing.
pub fn find_most_recent_user(root: &Path) -> Option<String> {
    let users_dir = root.join("users");
    let entries = std::fs::read_dir(&users_dir).ok()?;
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let user_id = entry.file_name().to_string_lossy().to_string();
        let db_path = entry.path().join("stats.db");
        if let Ok(meta) = std::fs::metadata(&db_path) {
            if let Ok(modified) = meta.modified() {
                if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                    newest = Some((modified, user_id));
                }
            }
        }
    }
    newest.map(|(_, uid)| uid)
}
