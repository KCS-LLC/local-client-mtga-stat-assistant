use crate::dlog;
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct CardDatabase {
    names: HashMap<u32, String>,
}

impl CardDatabase {
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(db) => {
                dlog!("[cards] loaded {} card names", db.names.len());
                db
            }
            Err(e) => {
                dlog!("[cards] failed to load card database: {} (continuing with empty cache; cards will display as Card #ID)", e);
                Self {
                    names: HashMap::new(),
                }
            }
        }
    }

    fn try_load() -> Result<Self, String> {
        let path = find_card_db_path()?;
        dlog!("[cards] opening {:?}", path);

        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("open: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT c.GrpId, l.Loc
                 FROM Cards c
                 JOIN Localizations_enUS l ON l.LocId = c.TitleId
                 WHERE l.Formatted = 1",
            )
            .map_err(|e| format!("prepare: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)? as u32, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("query: {}", e))?;

        let mut names = HashMap::new();
        for row in rows.flatten() {
            names.insert(row.0, row.1);
        }

        Ok(Self { names })
    }

    pub fn get_names(&self, grp_ids: &[u32]) -> HashMap<u32, String> {
        grp_ids
            .iter()
            .filter_map(|id| self.names.get(id).map(|n| (*id, n.clone())))
            .collect()
    }
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
