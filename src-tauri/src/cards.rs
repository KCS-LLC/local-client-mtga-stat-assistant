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
    lands: HashSet<u32>,
    /// Converted mana cost per card. Missing = 0.
    cmc: HashMap<u32, u32>,
    /// Human-readable mana cost string, e.g. "{2}{W}". Missing for lands and
    /// zero-cost cards that have no mana symbols.
    mana_cost: HashMap<u32, String>,
    /// Path of the .mtga file we loaded; used by the watcher to detect updates.
    /// None when no database was found (e.g. MTGA not yet installed).
    loaded_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct CardInfo {
    pub name: String,
    pub is_token: bool,
    pub is_land: bool,
    /// Converted mana cost / mana value. 0 when unknown or a land.
    pub cmc: u32,
    /// Human-readable mana cost, e.g. "{2}{W}". None for lands / zero-cost.
    pub mana_cost: Option<String>,
}

impl CardDatabase {
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(db) => {
                dlog!(
                    "[cards] loaded {} names ({} tokens, {} lands, {} cmc, {} mana_cost) from {:?}",
                    db.names.len(),
                    db.tokens.len(),
                    db.lands.len(),
                    db.cmc.len(),
                    db.mana_cost.len(),
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

        // The CMC column is Order_CMCWithXLast in current MTGA DB builds.
        // Fall back through older names just in case.
        let cmc_col: Option<String> =
            ["Order_CMCWithXLast", "CMC", "ManaValue"]
                .iter()
                .find_map(|&col| {
                    conn.query_row(
                        "SELECT 1 FROM pragma_table_info('Cards') WHERE name = ?1",
                        rusqlite::params![col],
                        |_| Ok(()),
                    )
                    .ok()
                    .map(|_| col.to_string())
                });

        // Mana cost is stored as OldSchoolManaText in the format "o2oW" —
        // each symbol prefixed with 'o'. We convert to "{2}{W}" at load time.
        let has_mana_cost_col = conn
            .query_row(
                "SELECT 1 FROM pragma_table_info('Cards') WHERE name = 'OldSchoolManaText'",
                [],
                |_| Ok(()),
            )
            .is_ok();

        let cmc_expr = cmc_col
            .as_deref()
            .map(|col| format!("COALESCE(c.{}, 0)", col))
            .unwrap_or_else(|| "0".to_string());
        let mana_cost_expr = if has_mana_cost_col {
            "COALESCE(c.OldSchoolManaText, '')"
        } else {
            "''"
        };

        let sql = format!(
            "SELECT c.GrpId, c.IsToken, c.Types, l.Loc, {}, {}
             FROM Cards c
             JOIN Localizations_enUS l ON l.LocId = c.TitleId
             WHERE l.Formatted = 1",
            cmc_expr, mana_cost_expr
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u32,
                    row.get::<_, bool>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)? as u32,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| format!("query: {}", e))?;

        let mut names = HashMap::new();
        let mut tokens = HashSet::new();
        let mut lands = HashSet::new();
        let mut cmc_map = HashMap::new();
        let mut mana_cost_map = HashMap::new();
        for row in rows.flatten() {
            let (grp, is_token, types, name, cmc, raw_cost) = row;
            names.insert(grp, strip_html_tags(&name));
            if is_token {
                tokens.insert(grp);
            }
            // Types is a comma-separated list of CardType enum integers.
            // 5 = Land. A card is a land if 5 appears anywhere in the list.
            if types.split(',').any(|t| t.trim() == "5") {
                lands.insert(grp);
            }
            if cmc > 0 {
                cmc_map.insert(grp, cmc);
            }
            if let Some(cost) = parse_mana_cost(&raw_cost) {
                mana_cost_map.insert(grp, cost);
            }
        }

        dlog!(
            "[cards] cmc_col={:?} has_mana_cost={} — {} cmc values, {} mana costs loaded",
            cmc_col,
            has_mana_cost_col,
            cmc_map.len(),
            mana_cost_map.len()
        );

        Ok(Self {
            names,
            tokens,
            lands,
            cmc: cmc_map,
            mana_cost: mana_cost_map,
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
                            is_land: self.lands.contains(id),
                            cmc: self.cmc.get(id).copied().unwrap_or(0),
                            mana_cost: self.mana_cost.get(id).cloned(),
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

        let current = cards.read().ok().and_then(|c| c.loaded_path.clone());

        let latest = match find_card_db_path() {
            Ok(p) => p,
            Err(_) => continue,
        };

        if Some(&latest) == current.as_ref() {
            continue;
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

/// Convert MTGA's internal mana cost format to standard notation.
/// MTGA stores mana cost as "o2oW" where each symbol is prefixed with 'o'.
/// Examples: "o2oW" → "{2}{W}", "oB" → "{B}", "o(R/G)" → "{R/G}"
/// Returns None for empty/land/zero-cost strings.
fn parse_mana_cost(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let result: String = raw
        .split('o')
        .filter(|s| !s.is_empty())
        .map(|sym| {
            // Hybrid mana stored as "(R/G)" — strip outer parens, brace it.
            let s = sym.trim_matches(|c| c == '(' || c == ')');
            format!("{{{}}}", s)
        })
        .collect();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Remove `<...>` HTML-style tags. MTGA stores names like
/// `<nobr>Wind-Scarred</nobr> Crag` to control line-wrapping in their UI.
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0u32;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn find_card_db_path() -> Result<PathBuf, String> {
    // MTGA installs under "Program Files" on whichever drive the user chose.
    // Scan C–Z so non-default install locations still work.
    for drive in b'C'..=b'Z' {
        let raw_dir = PathBuf::from(format!(
            "{}:/Program Files/Wizards of the Coast/MTGA/MTGA_Data/Downloads/Raw",
            drive as char
        ));
        let entries = match std::fs::read_dir(&raw_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Raw_CardDatabase_") && name.ends_with(".mtga") {
                return Ok(entry.path());
            }
        }
    }
    Err("no Raw_CardDatabase_*.mtga found — is MTGA installed?".to_string())
}

#[cfg(target_os = "macos")]
fn find_card_db_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let raw_dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.wizards.mtga")
        .join("Downloads")
        .join("Data");
    let entries = std::fs::read_dir(&raw_dir)
        .map_err(|e| format!("read_dir {:?}: {}", raw_dir, e))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("Raw_CardDatabase_") && name.ends_with(".mtga") {
            return Ok(entry.path());
        }
    }
    Err(format!("no Raw_CardDatabase_*.mtga in {:?}", raw_dir))
}
