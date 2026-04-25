use crate::dlog;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[derive(Default)]
pub struct CardDatabase {
    names: HashMap<u32, String>,
    tokens: HashSet<u32>,
    /// Path of the .mtga file we loaded; used by the watcher to detect updates.
    /// None when no database was found (e.g. MTGA not yet installed).
    loaded_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct CardInfo {
    pub name: String,
    pub is_token: bool,
}

impl CardDatabase {
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(db) => {
                dlog!(
                    "[cards] loaded {} names ({} tokens) from {:?}",
                    db.names.len(),
                    db.tokens.len(),
                    db.loaded_path
                );
                db
            }
            Err(e) => {
                dlog!(
                    "[cards] failed to load card database: {} (cards will display as Card #ID)",
                    e
                );
                Self::default()
            }
        }
    }

    fn try_load() -> Result<Self, String> {
        let path = find_card_db_path()?;
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("open: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT c.GrpId, c.IsToken, l.Loc
                 FROM Cards c
                 JOIN Localizations_enUS l ON l.LocId = c.TitleId
                 WHERE l.Formatted = 1",
            )
            .map_err(|e| format!("prepare: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u32,
                    row.get::<_, bool>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("query: {}", e))?;

        let mut names = HashMap::new();
        let mut tokens = HashSet::new();
        for row in rows.flatten() {
            let (grp, is_token, name) = row;
            names.insert(grp, name);
            if is_token {
                tokens.insert(grp);
            }
        }

        Ok(Self {
            names,
            tokens,
            loaded_path: Some(path),
        })
    }

    pub fn get_info(&self, grp_ids: &[u32]) -> HashMap<u32, CardInfo> {
        grp_ids
            .iter()
            .filter_map(|id| {
                self.names.get(id).map(|name| {
                    (
                        *id,
                        CardInfo {
                            name: name.clone(),
                            is_token: self.tokens.contains(id),
                        },
                    )
                })
            })
            .collect()
    }

    pub fn is_token(&self, grp_id: u32) -> bool {
        self.tokens.contains(&grp_id)
    }
}

pub type SharedCards = Arc<RwLock<CardDatabase>>;

/// Spawn a background thread that polls the MTGA install for a new
/// Raw_CardDatabase_*.mtga file (the hash in the filename changes on every
/// card-set update) and hot-reloads the in-memory map when one appears.
pub fn spawn_watcher(cards: SharedCards) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));

        let current = cards
            .read()
            .ok()
            .and_then(|c| c.loaded_path.clone());

        let latest = match find_card_db_path() {
            Ok(p) => p,
            Err(_) => continue, // MTGA not installed or directory unreadable
        };

        if Some(&latest) == current.as_ref() {
            continue; // unchanged
        }

        dlog!(
            "[cards] db file changed: {:?} -> {:?}; reloading",
            current,
            latest
        );
        let new_db = CardDatabase::load();
        if let Ok(mut guard) = cards.write() {
            *guard = new_db;
        }
    });
}

fn find_card_db_path() -> Result<PathBuf, String> {
    let raw_dir = PathBuf::from(
        "C:/Program Files/Wizards of the Coast/MTGA/MTGA_Data/Downloads/Raw",
    );
    let entries =
        std::fs::read_dir(&raw_dir).map_err(|e| format!("read_dir {:?}: {}", raw_dir, e))?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("Raw_CardDatabase_") && name.ends_with(".mtga") {
            return Ok(entry.path());
        }
    }

    Err(format!("no Raw_CardDatabase_*.mtga in {:?}", raw_dir))
}
