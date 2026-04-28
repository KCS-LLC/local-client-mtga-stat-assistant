use rusqlite::{params, Connection, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Serialize)]
pub struct DeckWL {
    pub deck_name: String,
    pub wins: u32,
    pub losses: u32,
}

#[derive(Debug, Serialize)]
pub struct MatchRecord {
    pub match_id: String,
    pub format: String,
    pub opponent_name: String,
    pub deck_id: Option<String>,
    pub deck_name: Option<String>,
    pub result: Option<String>,
    pub won_die_roll: Option<bool>,
    pub played_first: Option<bool>,
    pub started_at: i64,
}

#[derive(Debug, Serialize)]
pub struct DeckSnapshotRecord {
    pub deck_id: String,
    pub deck_name: String,
    /// grpId → quantity (commander is included here, not separated)
    pub cards: HashMap<u32, u32>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        db.seed_settings()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS matches (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                match_id         TEXT NOT NULL UNIQUE,
                format           TEXT NOT NULL,
                player_seat_id   INTEGER NOT NULL,
                player_team_id   INTEGER NOT NULL,
                opponent_name    TEXT NOT NULL,
                opponent_id      TEXT NOT NULL,
                player1_user_id  TEXT,
                player1_name     TEXT,
                player2_user_id  TEXT,
                player2_name     TEXT,
                deck_id          TEXT,
                deck_name        TEXT,
                deck_cards       TEXT,
                result           TEXT,
                won_die_roll     INTEGER,
                played_first     INTEGER,
                started_at       INTEGER NOT NULL,
                ended_at         INTEGER
            );

            CREATE TABLE IF NOT EXISTS games (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                match_id        TEXT NOT NULL,
                game_number     INTEGER NOT NULL,
                winning_team_id INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS opponent_cards (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                match_id    TEXT NOT NULL,
                game_number INTEGER NOT NULL,
                card_id     INTEGER NOT NULL,
                UNIQUE(match_id, game_number, card_id)
            );

            CREATE TABLE IF NOT EXISTS deck_snapshots (
                deck_id   TEXT PRIMARY KEY,
                deck_name TEXT NOT NULL,
                cards     TEXT NOT NULL
            );
            ",
        )?;

        // Additive migrations. SQLite has no IF NOT EXISTS for ADD COLUMN, so
        // check pragma_table_info first.
        let needed_columns = [
            "deck_cards",
            "player1_user_id",
            "player1_name",
            "player2_user_id",
            "player2_name",
        ];
        for col in &needed_columns {
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT 1 FROM pragma_table_info('matches') WHERE name = ?1",
                    params![col],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !exists {
                self.conn.execute(
                    &format!("ALTER TABLE matches ADD COLUMN {} TEXT", col),
                    [],
                )?;
            }
        }
        Ok(())
    }

    fn seed_settings(&self) -> Result<()> {
        let defaults = [
            ("backup_on_launch", "true"),
            ("track_deck_history", "true"),
            ("track_play_draw", "true"),
            ("track_flip_roll", "true"),
            ("player_id", ""),
        ];
        for (key, value) in &defaults {
            self.conn.execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        Ok(())
    }

    // --- settings ---

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok()
            .filter(|v: &String| !v.is_empty())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // --- matches ---

    #[allow(clippy::too_many_arguments)]
    pub fn insert_match(
        &self,
        match_id: &str,
        format: &str,
        player_seat_id: u8,
        player_team_id: u8,
        opponent_name: &str,
        opponent_id: &str,
        player1_user_id: &str,
        player1_name: &str,
        player2_user_id: &str,
        player2_name: &str,
        started_at: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO matches
             (match_id, format, player_seat_id, player_team_id, opponent_name, opponent_id,
              player1_user_id, player1_name, player2_user_id, player2_name, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                match_id,
                format,
                player_seat_id,
                player_team_id,
                opponent_name,
                opponent_id,
                player1_user_id,
                player1_name,
                player2_user_id,
                player2_name,
                started_at
            ],
        )?;
        Ok(())
    }

    /// Infer the local player's user_id from recent match history. Returns
    /// Some only when exactly one user_id appears in EVERY one of the last
    /// `window` matches (with canonical player1/player2 data) AND there are
    /// at least 2 matches AND the data shows at least 2 distinct user_ids
    /// total. Otherwise returns None (ambiguous — e.g., only one match seen,
    /// or the same opponent rematched without other data).
    ///
    /// Currently unused — the active design identifies the local user
    /// directly from log header lines via LocalPlayerIdentified, which is
    /// cheaper and works before any matches are recorded. Kept as a
    /// secondary signal in case header-line detection ever proves unreliable.
    #[allow(dead_code)]
    pub fn infer_local_user_id(&self, window: u32) -> Result<Option<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT player1_user_id, player1_name, player2_user_id, player2_name
             FROM matches
             WHERE player1_user_id IS NOT NULL AND player2_user_id IS NOT NULL
             ORDER BY started_at DESC
             LIMIT ?1",
        )?;
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map(params![window], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        if rows.len() < 2 {
            return Ok(None);
        }

        // Count how many matches each user_id appears in, and remember a
        // display name for it (first one we saw).
        let mut counts: HashMap<String, (u32, String)> = HashMap::new();
        let mut distinct_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (p1_id, p1_name, p2_id, p2_name) in &rows {
            distinct_ids.insert(p1_id.clone());
            distinct_ids.insert(p2_id.clone());
            counts
                .entry(p1_id.clone())
                .and_modify(|(c, _)| *c += 1)
                .or_insert((1, p1_name.clone()));
            counts
                .entry(p2_id.clone())
                .and_modify(|(c, _)| *c += 1)
                .or_insert((1, p2_name.clone()));
        }

        if distinct_ids.len() < 2 {
            return Ok(None);
        }

        let total = rows.len() as u32;
        let in_every: Vec<(&String, &String)> = counts
            .iter()
            .filter(|(_, (c, _))| *c == total)
            .map(|(uid, (_, name))| (uid, name))
            .collect();

        // Exactly one user_id present in every match → that's the local player.
        if in_every.len() == 1 {
            let (uid, name) = in_every[0];
            Ok(Some((uid.clone(), name.clone())))
        } else {
            Ok(None)
        }
    }

    /// Recent distinct players seen across matches (both seats), newest first.
    /// Used by the Settings UI to populate a "manual override" dropdown.
    pub fn get_recent_players(&self, window: u32) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT player1_user_id, player1_name, player2_user_id, player2_name, started_at
             FROM matches
             WHERE player1_user_id IS NOT NULL AND player2_user_id IS NOT NULL
             ORDER BY started_at DESC
             LIMIT ?1",
        )?;
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out: Vec<(String, String)> = Vec::new();
        let rows = stmt.query_map(params![window], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows.flatten() {
            for (uid, name) in [(row.0, row.1), (row.2, row.3)] {
                if seen.insert(uid.clone()) {
                    out.push((uid, name));
                }
            }
        }
        Ok(out)
    }

    pub fn finish_match(&self, match_id: &str, result: &str, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE matches SET result = ?1, ended_at = ?2 WHERE match_id = ?3",
            params![result, ended_at, match_id],
        )?;
        Ok(())
    }

    pub fn set_match_deck(
        &self,
        match_id: &str,
        deck_id: &str,
        deck_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE matches SET deck_id = ?1, deck_name = ?2 WHERE match_id = ?3",
            params![deck_id, deck_name, match_id],
        )?;
        Ok(())
    }

    /// Persist the actual deck composition seen for a match (from DeckLoaded).
    /// Stored as JSON of `grpId → quantity`. Lets us retroactively label the
    /// match later when a matching deck snapshot becomes available.
    pub fn set_match_deck_cards(
        &self,
        match_id: &str,
        cards: &HashMap<u32, u32>,
    ) -> Result<()> {
        let json = serde_json::to_string(cards).unwrap_or_else(|_| "{}".to_string());
        self.conn.execute(
            "UPDATE matches SET deck_cards = ?1 WHERE match_id = ?2",
            params![json, match_id],
        )?;
        Ok(())
    }

    /// Matches that haven't been correlated to a deck yet but have a captured
    /// composition. Returned newest-first so retroactive correlation reaches
    /// recent matches first (the most likely candidates).
    pub fn get_uncorrelated_matches(&self) -> Result<Vec<(String, HashMap<u32, u32>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT match_id, deck_cards FROM matches
             WHERE deck_id IS NULL AND deck_cards IS NOT NULL
             ORDER BY started_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let match_id: String = row.get(0)?;
            let cards_json: String = row.get(1)?;
            let cards: HashMap<u32, u32> =
                serde_json::from_str(&cards_json).unwrap_or_default();
            Ok((match_id, cards))
        })?;
        rows.collect()
    }

    // --- deck snapshots ---

    pub fn upsert_deck_snapshot(
        &self,
        deck_id: &str,
        deck_name: &str,
        cards: &HashMap<u32, u32>,
    ) -> Result<()> {
        let cards_json = serde_json::to_string(cards).unwrap_or_else(|_| "{}".to_string());
        self.conn.execute(
            "INSERT INTO deck_snapshots (deck_id, deck_name, cards) VALUES (?1, ?2, ?3)
             ON CONFLICT(deck_id) DO UPDATE SET
               deck_name = excluded.deck_name,
               cards = excluded.cards",
            params![deck_id, deck_name, cards_json],
        )?;
        Ok(())
    }

    pub fn get_deck_snapshots(&self) -> Result<HashMap<String, (String, HashMap<u32, u32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT deck_id, deck_name, cards FROM deck_snapshots")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let cards_json: String = row.get(2)?;
            let cards: HashMap<u32, u32> = serde_json::from_str(&cards_json).unwrap_or_default();
            Ok((id, (name, cards)))
        })?;
        rows.collect()
    }

    pub fn set_die_roll(&self, match_id: &str, won: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE matches SET won_die_roll = ?1 WHERE match_id = ?2",
            params![won as i32, match_id],
        )?;
        Ok(())
    }

    pub fn set_played_first(&self, match_id: &str, played_first: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE matches SET played_first = ?1 WHERE match_id = ?2",
            params![played_first as i32, match_id],
        )?;
        Ok(())
    }

    // --- games ---

    pub fn insert_game(
        &self,
        match_id: &str,
        game_number: u8,
        winning_team_id: u8,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO games (match_id, game_number, winning_team_id) VALUES (?1, ?2, ?3)",
            params![match_id, game_number, winning_team_id],
        )?;
        Ok(())
    }

    // --- opponent cards ---

    pub fn record_opponent_card(
        &self,
        match_id: &str,
        game_number: u8,
        card_id: u32,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO opponent_cards (match_id, game_number, card_id)
             VALUES (?1, ?2, ?3)",
            params![match_id, game_number, card_id],
        )?;
        Ok(())
    }

    // --- queries ---

    pub fn get_wl_stats(&self) -> Result<Vec<DeckWL>> {
        // Matches without a correlated deck still count toward W/L — they
        // collapse into a single "Unknown" row so the totals stay honest.
        // Retroactive correlation pulls them out of Unknown once the right
        // deck snapshot arrives.
        let mut stmt = self.conn.prepare(
            "SELECT
               COALESCE(deck_name, 'Unknown') AS name,
               SUM(CASE WHEN result = 'Win'  THEN 1 ELSE 0 END) AS wins,
               SUM(CASE WHEN result = 'Loss' THEN 1 ELSE 0 END) AS losses
             FROM matches
             WHERE result IS NOT NULL
             GROUP BY COALESCE(deck_name, 'Unknown')
             ORDER BY (wins + losses) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DeckWL {
                deck_name: row.get(0)?,
                wins: row.get(1)?,
                losses: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Wipe match-related data (matches, games, opponent_cards) but keep
    /// deck_snapshots (still useful for future correlation) and settings.
    pub fn reset_stats(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM opponent_cards;
             DELETE FROM games;
             DELETE FROM matches;",
        )?;
        Ok(())
    }

    pub fn get_match_history(&self, limit: u32) -> Result<Vec<MatchRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT match_id, format, opponent_name, deck_id, deck_name, result,
                    won_die_roll, played_first, started_at
             FROM matches
             ORDER BY started_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(MatchRecord {
                match_id: row.get(0)?,
                format: row.get(1)?,
                opponent_name: row.get(2)?,
                deck_id: row.get(3)?,
                deck_name: row.get(4)?,
                result: row.get(5)?,
                won_die_roll: row.get::<_, Option<i32>>(6)?.map(|v| v != 0),
                played_first: row.get::<_, Option<i32>>(7)?.map(|v| v != 0),
                started_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_decks(&self) -> Result<Vec<DeckSnapshotRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT deck_id, deck_name, cards FROM deck_snapshots ORDER BY deck_name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |row| {
            let deck_id: String = row.get(0)?;
            let deck_name: String = row.get(1)?;
            let cards_json: String = row.get(2)?;
            let cards: HashMap<u32, u32> =
                serde_json::from_str(&cards_json).unwrap_or_default();
            Ok(DeckSnapshotRecord {
                deck_id,
                deck_name,
                cards,
            })
        })?;
        rows.collect()
    }
}
