use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MatchPlayer {
    pub user_id: String,
    pub name: String,
    pub seat_id: u8,
    pub team_id: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeckCard {
    pub card_id: u32,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
    Command,
    Limbo,
    Unknown,
}

impl Zone {
    pub fn from_str(s: &str) -> Self {
        match s {
            "ZoneType_Library" => Zone::Library,
            "ZoneType_Hand" => Zone::Hand,
            "ZoneType_Battlefield" => Zone::Battlefield,
            "ZoneType_Graveyard" => Zone::Graveyard,
            "ZoneType_Exile" => Zone::Exile,
            "ZoneType_Stack" => Zone::Stack,
            "ZoneType_Command" => Zone::Command,
            "ZoneType_Limbo" => Zone::Limbo,
            _ => Zone::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum GameEvent {
    MatchStarted {
        match_id: String,
        player1: MatchPlayer,
        player2: MatchPlayer,
        format: String,
        timestamp: u64,
    },
    /// Synthesized by event_sink right after MatchStarted: tells the frontend
    /// which seat is the local player vs opponent, based on settings.player_id.
    /// MTGA assigns seats arbitrarily so the frontend can't infer this on its own.
    PlayerIdentified {
        player_seat_id: u8,
        opponent_seat_id: u8,
        opponent: MatchPlayer,
    },
    MatchEnded {
        match_id: String,
        winning_team_id: u8,
        reason: String,
        timestamp: u64,
    },
    /// Deck submitted for the current match, from ConnectResp.deckMessage
    DeckLoaded {
        cards: Vec<DeckCard>,
        commander: Option<u32>,
    },
    /// A card moved between zones
    ZoneChanged {
        instance_id: u32,
        card_id: u32,
        from_zone: Zone,
        to_zone: Zone,
        owner_seat_id: u8,
        face_down: bool,
    },
    /// Full deck snapshot parsed from MTGA's CourseData log block; persisted
    /// to the deck_snapshots table for later correlation against in-match decks.
    DeckSnapshot {
        deck_id: String,
        deck_name: String,
        cards: Vec<DeckCard>,
    },
    /// A single game within a match ended (Bo3 only for sideboard_next: true)
    GameEnded {
        winning_team_id: u8,
        game_number: u8,
        /// true = sideboard phase follows (more games remain)
        /// false = match is over
        sideboard_next: bool,
    },
    /// One player's die roll result at match start (one event emitted per player)
    DieRollResult {
        seat_id: u8,
        roll_value: u32,
    },
    /// Commander revealed in the Command zone (visible from game start)
    CommanderRevealed {
        card_id: u32,
        seat_id: u8,
    },
    /// Commander cast from the command zone; tax = extra mana cost above base
    CommanderCast {
        card_id: u32,
        seat_id: u8,
        cast_count: u8,
        tax: u8,
    },
    /// Commander moved back to the command zone (died or exiled, player chose command zone)
    CommanderReturned {
        card_id: u32,
        seat_id: u8,
    },
    /// A player's library was shuffled; instance IDs for that library are now stale
    LibraryShuffle {
        seat_id: u8,
    },
    /// Turn 1 of game 1 has begun; seat_id is who plays first this match.
    /// Emitted once per match from the GRE turnInfo signal, which is more
    /// accurate than inferring from the die roll (winner can choose to go second).
    PlayedFirst {
        seat_id: u8,
    },
    /// Snapshot of which cards (grpIds) are currently in non-library zones for
    /// a seat, emitted after each Full GameStateMessage. Lets the frontend
    /// recompute library state without needing to have caught every individual
    /// Library→Hand/Battlefield/etc transition (the opening hand bypasses
    /// per-card ZoneTransfer annotations entirely).
    ZoneStateSync {
        seat_id: u8,
        hand: Vec<u32>,
        battlefield: Vec<u32>,
        graveyard: Vec<u32>,
        exile: Vec<u32>,
        stack: Vec<u32>,
        /// grpId of the top of this seat's library when its identity is
        /// known to us (e.g. revealed by a scry/surveil/tutor effect). None
        /// when the top is uniformly random — the normal case.
        top_of_library: Option<u32>,
    },
    /// Local MTGA user identified from the log header pattern
    /// `Match to <user_id>:` / `<user_id> to Match:`. Tells the event_sink
    /// which per-user database to write to. Fires whenever a fresh user_id
    /// is observed (login, account switch). Quiet for every subsequent header
    /// line that names the same user.
    LocalPlayerIdentified {
        user_id: String,
    },
}
