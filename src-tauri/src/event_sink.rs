use crate::cards::CardDatabase;
use crate::db::Db;
use crate::events::{GameEvent, Zone};
use std::collections::HashMap;

pub struct EventSink {
    current_match_id: Option<String>,
    player_seat_id: u8,
    player_team_id: u8,
    opponent_seat_id: u8,
    current_game_number: u8,
    die_rolls: HashMap<u8, u32>,
    // deck_id → (deck_name, card_id → quantity); rebuilt as DeckSnapshot events arrive
    deck_snapshots: HashMap<String, (String, HashMap<u32, u32>)>,
    // True once we've successfully matched the current match's deck
    deck_identified: bool,
}

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
            deck_identified: false,
        }
    }

    pub fn process(
        &mut self,
        event: &GameEvent,
        db: &mut Db,
        cards: &CardDatabase,
    ) -> Vec<GameEvent> {
        let mut emit = vec![];
        match event {
            GameEvent::MatchStarted {
                match_id,
                player1,
                player2,
                format,
                timestamp,
            } => {
                // Identify which player is "us" from stored player_id
                let saved_id = db.get_setting("player_id");

                let (me, opponent) = match &saved_id {
                    Some(id) if player1.user_id == *id => (player1, player2),
                    Some(id) if player2.user_id == *id => (player2, player1),
                    // Unknown — assume player1; save for future matches
                    _ => {
                        let _ = db.set_setting("player_id", &player1.user_id);
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
                    *timestamp as i64,
                );

                // Tell the frontend which seat is the local player
                emit.push(GameEvent::PlayerIdentified {
                    player_seat_id: me.seat_id,
                    opponent_seat_id: opponent.seat_id,
                    opponent: opponent.clone(),
                });
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

                // Once we have rolls for both players, determine who won
                if self.die_rolls.len() >= 2 {
                    if let Some(mid) = &self.current_match_id.clone() {
                        let player_roll = self.die_rolls.get(&self.player_seat_id).copied().unwrap_or(0);
                        let opponent_roll = self.die_rolls.get(&self.opponent_seat_id).copied().unwrap_or(0);
                        let won = player_roll > opponent_roll;
                        let _ = db.set_die_roll(mid, won);
                        // Assume die roll winner plays first
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
                // Record real opponent cards (skip tokens — they don't exist
                // in the deck list and would pollute the per-match history)
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
                self.deck_snapshots
                    .insert(deck_id.clone(), (deck_name.clone(), qty));
            }

            GameEvent::DeckLoaded { cards, commander } => {
                if !self.deck_identified {
                    if let Some(mid) = self.current_match_id.clone() {
                        // DeckSnapshot lists the commander as part of `cards`,
                        // but DeckLoaded keeps it separate, so add it here
                        // before comparing or no Brawl/Commander deck will match.
                        let mut loaded: HashMap<u32, u32> =
                            cards.iter().map(|c| (c.card_id, c.quantity)).collect();
                        if let Some(cmdr) = commander {
                            loaded.entry(*cmdr).or_insert(1);
                        }
                        if let Some((deck_id, name)) = self
                            .deck_snapshots
                            .iter()
                            .find(|(_, (_, snap_cards))| *snap_cards == loaded)
                            .map(|(id, (name, _))| (id.clone(), name.clone()))
                        {
                            let _ = db.set_match_deck(&mid, &deck_id, &name);
                            self.deck_identified = true;
                        }
                    }
                }
            }

            // Not persisted at this layer: CommanderCast, CommanderReturned,
            // CommanderRevealed, LibraryShuffle (frontend-only)
            _ => {}
        }
        emit
    }
}
