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
        card_id: u32,
        from_zone: Zone,
        to_zone: Zone,
        owner_seat_id: u8,
        face_down: bool,
    },
    /// Full deck snapshot from session startup (only emitted when track_deck_history is on)
    DeckSnapshot {
        deck_id: String,
        deck_name: String,
        cards: Vec<DeckCard>,
    },
}
