use crate::cards::CardDatabase;
use crate::db::Db;
use crate::db_hub::DbHub;
use crate::dlog;
use crate::events::{GameEvent, Zone};
use std::collections::HashMap;

pub struct EventSink {
    current_match_id: Option<String>,
    player_seat_id: u8,
    player_team_id: u8,
    opponent_seat_id: u8,
    current_game_number: u8,
    die_rolls: HashMap<u8, u32>,
    // deck_id → (deck_name, card_id → quantity); rebuilt from the active DB
    // each time we switch users (different users have different decks)
    deck_snapshots: HashMap<String, (String, HashMap<u32, u32>)>,
    // Most recently loaded deck (cards merged with commander). Survives across
    // the MatchStarted boundary because DeckLoaded (from ConnectResp) typically
    // fires *before* MatchStarted (from MatchGameRoomStateChange).
    last_loaded_deck: Option<HashMap<u32, u32>>,
    // grpId of the local player's commander (from DeckLoaded.commander). Used
    // to synthesize a CommanderRevealed for our side immediately on MatchStarted,
    // since the gameObject scan in detect_new_commanders may miss the initial
    // Full snapshot if the tailer started mid-session.
    last_loaded_commander: Option<u32>,
    // True once we've successfully matched the current match's deck
    deck_identified: bool,
    // DB-bound events received while no user was active (e.g. lobby-time
    // CourseData → DeckSnapshot). Drained into the new DB the moment a user
    // is identified and the hub swaps. Capped to keep a stuck "no user" loop
    // from growing without bound.
    pending: Vec<GameEvent>,
}

const PENDING_CAP: usize = 1024;

impl EventSink {
    pub fn new() -> Self {
        Self {
            current_match_id: None,
            player_seat_id: 0,
            player_team_id: 0,
            opponent_seat_id: 0,
            current_game_number: 1,
            die_rolls: HashMap::new(),
            deck_snapshots: HashMap::new(),
            last_loaded_deck: None,
            last_loaded_commander: None,
            deck_identified: false,
            pending: Vec::new(),
        }
    }


    fn try_correlate_deck(&mut self, match_id: &str, db: &mut Db) {
        if self.deck_identified {
            return;
        }
        let deck = match &self.last_loaded_deck {
            Some(d) => d,
            None => return,
        };
        let matched = self
            .deck_snapshots
            .iter()
            .find(|(_, (_, snap_cards))| snap_cards == deck)
            .map(|(id, (name, _))| (id.clone(), name.clone()));
        if let Some((deck_id, name)) = matched {
            let _ = db.set_match_deck(match_id, &deck_id, &name);
            self.deck_identified = true;
        }
    }

    /// Walk every previously-recorded match that's still missing a deck and
    /// label any whose stored composition matches the just-upserted snapshot.
    /// Runs after each DeckSnapshot. Catches the case where the user changed
    /// the deck in MTGA and played a match before MTGA wrote a fresh
    /// CourseData block — the older match composition would only become
    /// identifiable once the new snapshot arrived.
    fn retroactively_correlate(
        &self,
        deck_id: &str,
        deck_name: &str,
        snapshot_cards: &HashMap<u32, u32>,
        db: &mut Db,
    ) {
        let candidates = match db.get_uncorrelated_matches() {
            Ok(c) => c,
            Err(_) => return,
        };
        for (match_id, cards) in candidates {
            if cards == *snapshot_cards {
                let _ = db.set_match_deck(&match_id, deck_id, deck_name);
            }
        }
    }

    pub fn process(
        &mut self,
        event: &GameEvent,
        hub: &mut DbHub,
        cards: &CardDatabase,
    ) -> Vec<GameEvent> {
        let mut emit = vec![];

        // LocalPlayerIdentified routes ahead of every other event: it tells
        // us which user's DB the rest of the events should land in. After a
        // switch we reload deck snapshots from the new DB and persist the
        // user_id as the player_id setting (used by MatchStarted to pick
        // "us" out of player1/player2).
        if let GameEvent::LocalPlayerIdentified { user_id } = event {
            match hub.switch(user_id) {
                Ok(true) => {
                    if let Some(db) = hub.db() {
                        self.deck_snapshots = db.get_deck_snapshots().unwrap_or_default();
                    }
                    if let Some(db) = hub.db_mut() {
                        let _ = db.set_setting("player_id", user_id);
                    }
                    self.current_match_id = None;
                    self.last_loaded_deck = None;
                    self.last_loaded_commander = None;
                    self.deck_identified = false;

                    // Drain anything that arrived before the user was known
                    // (e.g. lobby-time DeckSnapshots) into the now-active DB.
                    // We discard their emit returns: the frontend already saw
                    // these events the first time around.
                    let pending = std::mem::take(&mut self.pending);
                    if !pending.is_empty() {
                        dlog!(
                            "[event_sink] draining {} buffered event(s) into {}'s DB",
                            pending.len(),
                            user_id
                        );
                        for evt in pending {
                            let _ = self.process(&evt, hub, cards);
                        }
                    }
                }
                Ok(false) => {} // already on this user
                Err(e) => dlog!("[event_sink] switch to {} failed: {}", user_id, e),
            }
            return emit;
        }

        let db = match hub.db_mut() {
            Some(d) => d,
            None => {
                // No user identified yet — buffer this event so it can be
                // replayed once LocalPlayerIdentified arrives. Cap the buffer
                // so a permanently-unidentified session can't grow memory
                // without bound; the oldest events drop first.
                if self.pending.len() >= PENDING_CAP {
                    self.pending.remove(0);
                }
                self.pending.push(event.clone());
                return emit;
            }
        };

        match event {
            GameEvent::MatchStarted {
                match_id,
                player1,
                player2,
                format,
                timestamp,
            } => {
                // player_id is set authoritatively from log headers via
                // LocalPlayerIdentified. If for any reason the saved id
                // doesn't match either player (shouldn't happen), fall back
                // to player1 so the match still gets recorded.
                let saved_id = db.get_setting("player_id");
                let (me, opponent) = match &saved_id {
                    Some(id) if player1.user_id == *id => (player1, player2),
                    Some(id) if player2.user_id == *id => (player2, player1),
                    _ => {
                        dlog!(
                            "[event_sink] no player_id match in MatchStarted ({:?} vs {} / {}); defaulting player1",
                            saved_id,
                            player1.user_id,
                            player2.user_id
                        );
                        (player1, player2)
                    }
                };

                self.current_match_id = Some(match_id.clone());
                self.player_seat_id = me.seat_id;
                self.player_team_id = me.team_id;
                self.opponent_seat_id = opponent.seat_id;
                self.current_game_number = 1;
                self.die_rolls.clear();
                self.deck_identified = false;

                let _ = db.insert_match(
                    match_id,
                    format,
                    me.seat_id,
                    me.team_id,
                    &opponent.name,
                    &opponent.user_id,
                    &player1.user_id,
                    &player1.name,
                    &player2.user_id,
                    &player2.name,
                    *timestamp as i64,
                );

                if let Some(deck) = &self.last_loaded_deck {
                    let _ = db.set_match_deck_cards(match_id, deck);
                }
                self.try_correlate_deck(match_id, db);

                emit.push(GameEvent::PlayerIdentified {
                    player_seat_id: me.seat_id,
                    opponent_seat_id: opponent.seat_id,
                    opponent: opponent.clone(),
                });

                if let Some(cmdr) = self.last_loaded_commander {
                    emit.push(GameEvent::CommanderRevealed {
                        card_id: cmdr,
                        seat_id: me.seat_id,
                    });
                }
            }

            GameEvent::MatchEnded {
                match_id,
                winning_team_id,
                timestamp,
                ..
            } => {
                let result = if *winning_team_id == self.player_team_id {
                    "Win"
                } else {
                    "Loss"
                };
                let _ = db.finish_match(match_id, result, *timestamp as i64);
                self.current_match_id = None;
            }

            GameEvent::GameEnded {
                winning_team_id,
                game_number,
                ..
            } => {
                if let Some(mid) = &self.current_match_id {
                    let _ = db.insert_game(mid, *game_number, *winning_team_id);
                }
                self.current_game_number = game_number + 1;
            }

            GameEvent::DieRollResult { seat_id, roll_value } => {
                self.die_rolls.insert(*seat_id, *roll_value);

                if self.die_rolls.len() >= 2 {
                    if let Some(mid) = &self.current_match_id.clone() {
                        let player_roll = self.die_rolls.get(&self.player_seat_id).copied().unwrap_or(0);
                        let opponent_roll = self.die_rolls.get(&self.opponent_seat_id).copied().unwrap_or(0);
                        let won = player_roll > opponent_roll;
                        let _ = db.set_die_roll(mid, won);
                        let _ = db.set_played_first(mid, won);
                    }
                }
            }

            GameEvent::ZoneChanged {
                card_id,
                to_zone,
                owner_seat_id,
                face_down,
                ..
            } => {
                if matches!(to_zone, Zone::Battlefield)
                    && *owner_seat_id == self.opponent_seat_id
                    && !face_down
                    && self.current_match_id.is_some()
                    && !cards.is_token(*card_id)
                {
                    let mid = self.current_match_id.clone().unwrap();
                    let _ = db.record_opponent_card(&mid, self.current_game_number, *card_id);
                }
            }

            GameEvent::DeckSnapshot {
                deck_id,
                deck_name,
                cards,
            } => {
                let qty: HashMap<u32, u32> =
                    cards.iter().map(|c| (c.card_id, c.quantity)).collect();
                let _ = db.upsert_deck_snapshot(deck_id, deck_name, &qty);
                self.deck_snapshots
                    .insert(deck_id.clone(), (deck_name.clone(), qty.clone()));
                self.retroactively_correlate(deck_id, deck_name, &qty, db);
            }

            GameEvent::DeckLoaded { cards, commander } => {
                let mut deck: HashMap<u32, u32> =
                    cards.iter().map(|c| (c.card_id, c.quantity)).collect();
                if let Some(cmdr) = commander {
                    deck.entry(*cmdr).or_insert(1);
                }
                self.last_loaded_deck = Some(deck.clone());
                self.last_loaded_commander = *commander;

                if let Some(mid) = self.current_match_id.clone() {
                    let _ = db.set_match_deck_cards(&mid, &deck);
                    self.try_correlate_deck(&mid, db);
                }
            }

            GameEvent::PlayedFirst { seat_id } => {
                if let Some(mid) = &self.current_match_id.clone() {
                    let went_first = *seat_id == self.player_seat_id;
                    let _ = db.set_played_first(mid, went_first);
                    dlog!("[event_sink] played_first: seat {} went first, player_seat={}, went_first={}", seat_id, self.player_seat_id, went_first);
                }
            }

            // Not persisted at this layer: CommanderCast, CommanderReturned,
            // CommanderRevealed, LibraryShuffle (frontend-only). Also no
            // LocalPlayerIdentified — that's handled above before db lookup.
            _ => {}
        }
        emit
    }
}
