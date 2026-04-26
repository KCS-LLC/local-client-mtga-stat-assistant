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
    pub deck_name: Option<String>,
    pub result: Option<String>,
    pub won_die_roll: Option<bool>,
    pub played_first: Option<bool>,
    pub started_at: i64,
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
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                match_id        TEXT NOT NULL UNIQUE,
                format          TEXT NOT NULL,
                player_seat_id  INTEGER NOT NULL,
                player_team_id  INTEGER NOT NULL,
                opponent_name   TEXT NOT NULL,
                opponent_id     TEXT NOT NULL,
                deck_id         TEXT,
                deck_name       TEXT,
                result          TEXT,
                won_die_roll    INTEGER,
                played_first    INTEGER,
                started_at      INTEGER NOT NULL,
                ended_at        INTEGER
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
        )
    }

    fn seed_settings(&self) -> Result<()> {
        let defaults = [
            ("backup_on_launch", "true"),
            ("track_deck_history", "false"),
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

    pub fn insert_match(
        &self,
        match_id: &str,
        format: &str,
        player_seat_id: u8,
        player_team_id: u8,
        opponent_name: &str,
        opponent_id: &str,
        started_at: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO matches
             (match_id, format, player_seat_id, player_team_id, opponent_name, opponent_id, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                match_id,
                format,
                player_seat_id,
                player_team_id,
                opponent_name,
                opponent_id,
                started_at
            ],
        )?;
        Ok(())
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
        // Only group identified decks. Matches with NULL deck_name (correlation
        // failed or pre-correlation data) are not aggregated here — they would
        // all merge under one bucket and obscure real per-deck stats.
        let mut stmt = self.conn.prepare(
            "SELECT
               deck_name AS name,
               SUM(CASE WHEN result = 'Win'  THEN 1 ELSE 0 END) AS wins,
               SUM(CASE WHEN result = 'Loss' THEN 1 ELSE 0 END) AS losses
             FROM matches
             WHERE result IS NOT NULL AND deck_name IS NOT NULL
             GROUP BY deck_name
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
            "SELECT match_id, format, opponent_name, deck_name, result,
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
                deck_name: row.get(3)?,
                result: row.get(4)?,
                won_die_roll: row.get::<_, Option<i32>>(5)?.map(|v| v != 0),
                played_first: row.get::<_, Option<i32>>(6)?.map(|v| v != 0),
                started_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }
}
