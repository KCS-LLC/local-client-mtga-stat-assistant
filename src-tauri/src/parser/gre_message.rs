use crate::dlog;
use crate::events::{DeckCard, GameEvent, Zone};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

struct ZoneInfo {
    zone_type: String,
    owner_seat_id: u8,
}

pub struct GreParser {
    // Maps zoneId → zone metadata; rebuilt on each Full snapshot
    zone_map: HashMap<u32, ZoneInfo>,
    // Maps instanceId → grpId; updated incrementally
    instance_map: HashMap<u32, u32>,
    // Maps instanceId → current visibility
    visibility_map: HashMap<u32, String>,
    // Maps instanceId → owner seat id (cards' true owner, used for ZoneChanged
    // events because shared zones like Stack and Command report ownerSeatId: 0)
    owner_map: HashMap<u32, u8>,
    // Current game number within the match (1-indexed, increments on GameEnded)
    game_number: u8,
    // Maps commander grpId → number of times cast this match (for tax calculation)
    commander_casts: HashMap<u32, u8>,
    // (seat_id, grpId) pairs for commanders we've already announced
    known_commanders: HashSet<(u8, u32)>,
}

impl GreParser {
    pub fn new() -> Self {
        Self {
            zone_map: HashMap::new(),
            instance_map: HashMap::new(),
            visibility_map: HashMap::new(),
            owner_map: HashMap::new(),
            game_number: 1,
            commander_casts: HashMap::new(),
            known_commanders: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        self.zone_map.clear();
        self.instance_map.clear();
        self.visibility_map.clear();
        self.owner_map.clear();
        self.game_number = 1;
        self.commander_casts.clear();
        self.known_commanders.clear();
    }

    pub fn parse(&mut self, content: &str) -> Vec<GameEvent> {
        // MTGA occasionally writes two JSON objects back-to-back in the same
        // chunk (no [UnityCrossThreadLogger] marker between them). serde_json
        // ::from_str only parses one root value, so we'd silently lose every
        // message in the second JSON. Use a streaming Deserializer instead.
        let mut events = vec![];
        let stream = serde_json::Deserializer::from_str(content).into_iter::<Value>();
        for v_result in stream {
            match v_result {
                Ok(v) => events.extend(self.process_root_value(&v)),
                Err(e) => {
                    dlog!(
                        "[gre] JSON parse stopped at byte ~{}: {}",
                        e.column(),
                        e
                    );
                    break;
                }
            }
        }
        events
    }

    /// Process a single greToClientEvent root JSON object and return events
    /// produced by its messages.
    fn process_root_value(&mut self, v: &Value) -> Vec<GameEvent> {
        let messages = match v
            .get("greToClientEvent")
            .and_then(|e| e.get("greToClientMessages"))
            .and_then(|m| m.as_array())
        {
            Some(m) => m,
            None => return vec![],
        };

        let mut events = vec![];
        for msg in messages {
            let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match msg_type {
                "GREMessageType_ConnectResp" => {
                    if let Some(e) = self.parse_connect_resp(msg) {
                        events.push(e);
                    }
                }
                "GREMessageType_SubmitDeckReq" => {
                    if let Some(e) = self.parse_submit_deck(msg) {
                        events.push(e);
                    }
                }
                "GREMessageType_GameStateMessage" => {
                    events.extend(self.parse_game_state(msg));
                }
                "GREMessageType_IntermissionReq" => {
                    if let Some(e) = self.parse_intermission(msg) {
                        events.push(e);
                    }
                }
                "GREMessageType_DieRollResultsResp" => {
                    events.extend(self.parse_die_rolls(msg));
                }
                _ => {}
            }
        }
        events
    }

    fn parse_connect_resp(&self, msg: &Value) -> Option<GameEvent> {
        let deck_msg = msg.get("connectResp")?.get("deckMessage")?;
        flat_array_to_deck_loaded(deck_msg)
    }

    fn parse_game_state(&mut self, msg: &Value) -> Vec<GameEvent> {
        let gs = match msg.get("gameStateMessage") {
            Some(gs) => gs,
            None => return vec![],
        };

        let update_type = gs.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if update_type == "GameStateType_Full" {
            self.rebuild_maps(gs);
            return self.detect_new_commanders(gs);
        }

        self.update_maps(gs);
        let mut events = self.detect_new_commanders(gs);
        events.extend(self.process_zone_transfers(gs));
        events
    }

    /// Scan gameObjects for any card currently in a Command zone, and emit a
    /// CommanderRevealed event for each one we haven't announced yet.
    fn detect_new_commanders(&mut self, gs: &Value) -> Vec<GameEvent> {
        let objects = match gs.get("gameObjects").and_then(|o| o.as_array()) {
            Some(o) => o,
            None => return vec![],
        };
        let mut events = vec![];
        for obj in objects {
            let zone_id = match obj.get("zoneId").and_then(|z| z.as_u64()) {
                Some(z) => z as u32,
                None => continue,
            };
            let zone = match self.zone_map.get(&zone_id) {
                Some(z) => z,
                None => continue,
            };
            if zone.zone_type != "ZoneType_Command" {
                continue;
            }
            let grp_id = match obj.get("grpId").and_then(|g| g.as_u64()) {
                Some(g) => g as u32,
                None => continue,
            };
            let owner = match obj.get("ownerSeatId").and_then(|o| o.as_u64()) {
                Some(o) => o as u8,
                None => continue,
            };
            if self.known_commanders.insert((owner, grp_id)) {
                events.push(GameEvent::CommanderRevealed {
                    card_id: grp_id,
                    seat_id: owner,
                });
            }
        }
        events
    }

    fn rebuild_maps(&mut self, gs: &Value) {
        self.zone_map.clear();
        self.instance_map.clear();
        self.visibility_map.clear();
        self.owner_map.clear();

        if let Some(zones) = gs.get("zones").and_then(|z| z.as_array()) {
            for zone in zones {
                self.ingest_zone(zone);
            }
        }
        if let Some(objects) = gs.get("gameObjects").and_then(|o| o.as_array()) {
            for obj in objects {
                self.ingest_object(obj);
            }
        }
    }

    fn update_maps(&mut self, gs: &Value) {
        if let Some(zones) = gs.get("zones").and_then(|z| z.as_array()) {
            for zone in zones {
                self.ingest_zone(zone);
            }
        }
        if let Some(objects) = gs.get("gameObjects").and_then(|o| o.as_array()) {
            for obj in objects {
                self.ingest_object(obj);
            }
        }
    }

    fn ingest_zone(&mut self, zone: &Value) {
        if let (Some(id), Some(zone_type)) = (
            zone.get("zoneId").and_then(|z| z.as_u64()),
            zone.get("type").and_then(|t| t.as_str()),
        ) {
            let owner_seat_id = zone
                .get("ownerSeatId")
                .and_then(|o| o.as_u64())
                .unwrap_or(0) as u8;

            self.zone_map.insert(
                id as u32,
                ZoneInfo {
                    zone_type: zone_type.to_string(),
                    owner_seat_id,
                },
            );
        }
    }

    fn ingest_object(&mut self, obj: &Value) {
        if let Some(instance_id) = obj.get("instanceId").and_then(|i| i.as_u64()) {
            let id = instance_id as u32;
            if let Some(grp_id) = obj.get("grpId").and_then(|g| g.as_u64()) {
                self.instance_map.insert(id, grp_id as u32);
            }
            if let Some(owner) = obj.get("ownerSeatId").and_then(|o| o.as_u64()) {
                self.owner_map.insert(id, owner as u8);
            }
            let vis = obj
                .get("visibility")
                .and_then(|v| v.as_str())
                .unwrap_or("Visibility_Public")
                .to_string();
            self.visibility_map.insert(id, vis);
        }
    }

    fn process_zone_transfers(&mut self, gs: &Value) -> Vec<GameEvent> {
        let annotations = match gs.get("annotations").and_then(|a| a.as_array()) {
            Some(a) => a,
            None => return vec![],
        };

        let mut events = vec![];
        for ann in annotations {
            let types = match ann.get("type").and_then(|t| t.as_array()) {
                Some(t) => t,
                None => continue,
            };
            let type_strs: Vec<&str> = types.iter().filter_map(|t| t.as_str()).collect();
            if type_strs.contains(&"AnnotationType_ZoneTransfer") {
                events.extend(self.handle_zone_transfer(ann));
            } else if type_strs.contains(&"AnnotationType_Shuffle") {
                events.extend(self.handle_shuffle(ann));
            }
        }
        events
    }

    fn handle_shuffle(&mut self, ann: &Value) -> Vec<GameEvent> {
        let details = match ann.get("details").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return vec![],
        };

        if let Some(old_ids) = details.iter().find_map(|d| {
            if d.get("key")?.as_str()? == "OldIds" {
                d.get("valueInt32")?.as_array()
            } else {
                None
            }
        }) {
            for id in old_ids {
                if let Some(id) = id.as_u64() {
                    let id = id as u32;
                    self.instance_map.remove(&id);
                    self.visibility_map.remove(&id);
                    self.owner_map.remove(&id);
                }
            }
        }

        let seat_id = ann
            .get("affectedIds")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|id| id.as_u64())
            .unwrap_or(0) as u8;

        vec![GameEvent::LibraryShuffle { seat_id }]
    }

    fn handle_zone_transfer(&mut self, ann: &Value) -> Vec<GameEvent> {
        let instance_id = match ann
            .get("affectedIds")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|id| id.as_u64())
        {
            Some(id) => id as u32,
            None => return vec![],
        };

        let details = match ann.get("details").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return vec![],
        };

        let src_zone_id = match extract_int_detail(details, "zone_src") {
            Some(id) => id as u32,
            None => return vec![],
        };
        let dst_zone_id = match extract_int_detail(details, "zone_dest") {
            Some(id) => id as u32,
            None => return vec![],
        };

        let (src_zone_type, src_zone_owner) = match self.zone_map.get(&src_zone_id) {
            Some(info) => (info.zone_type.clone(), info.owner_seat_id),
            None => return vec![],
        };
        let dst_zone_type = match self.zone_map.get(&dst_zone_id) {
            Some(info) => info.zone_type.clone(),
            None => return vec![],
        };

        let card_id = match self.instance_map.get(&instance_id) {
            Some(&id) => id,
            None => return vec![],
        };

        // Prefer the card's own ownerSeatId (from gameObject) — shared zones
        // like Stack and Command report ownerSeatId: 0 on the zone itself.
        let owner_seat_id = self
            .owner_map
            .get(&instance_id)
            .copied()
            .filter(|&o| o != 0)
            .unwrap_or(src_zone_owner);

        let face_down = dst_zone_type == "ZoneType_Exile"
            && self
                .visibility_map
                .get(&instance_id)
                .map(|v| v != "Visibility_Public")
                .unwrap_or(false);

        let mut events = vec![GameEvent::ZoneChanged {
            instance_id,
            card_id,
            from_zone: Zone::from_str(&src_zone_type),
            to_zone: Zone::from_str(&dst_zone_type),
            owner_seat_id,
            face_down,
        }];

        if src_zone_type == "ZoneType_Command" && dst_zone_type == "ZoneType_Stack" {
            let cast_count = {
                let entry = self.commander_casts.entry(card_id).or_insert(0);
                *entry += 1;
                *entry
            };
            // tax is the additional cost for the NEXT cast attempt — i.e. what
            // it'll cost if the commander dies and is recast. After 1 cast,
            // next cast costs +2. After 2 casts, +4. This way the UI can show
            // the upcoming penalty as soon as the commander leaves the
            // battlefield, not only after the recast.
            let tax = cast_count * 2;
            events.push(GameEvent::CommanderCast {
                card_id,
                seat_id: owner_seat_id,
                cast_count,
                tax,
            });
        }

        // Commander return via state-based action: GY/Exile → Command
        if dst_zone_type == "ZoneType_Command"
            && (src_zone_type == "ZoneType_Graveyard" || src_zone_type == "ZoneType_Exile")
            && self.commander_casts.contains_key(&card_id)
        {
            events.push(GameEvent::CommanderReturned {
                card_id,
                seat_id: owner_seat_id,
            });
        }

        events
    }

    fn parse_submit_deck(&self, msg: &Value) -> Option<GameEvent> {
        let deck = msg.get("submitDeckReq")?.get("deck")?;
        flat_array_to_deck_loaded(deck)
    }

    fn parse_intermission(&mut self, msg: &Value) -> Option<GameEvent> {
        let req = msg.get("intermissionReq")?;
        let prompt_id = req
            .get("intermissionPrompt")?
            .get("promptId")?
            .as_u64()?;

        let winning_team_id = req
            .get("intermissionPrompt")?
            .get("parameters")?
            .as_array()?
            .iter()
            .find_map(|p| {
                if p.get("parameterName")?.as_str()? == "WinningTeamId" {
                    p.get("numberValue")?.as_u64()
                } else {
                    None
                }
            })? as u8;

        // promptId 25 = game over, sideboard follows; promptId 27 = match over
        let sideboard_next = prompt_id == 25;
        let game_number = self.game_number;

        if sideboard_next {
            self.game_number += 1;
        }

        Some(GameEvent::GameEnded {
            winning_team_id,
            game_number,
            sideboard_next,
        })
    }

    fn parse_die_rolls(&self, msg: &Value) -> Vec<GameEvent> {
        let rolls = msg
            .get("dieRollResultsResp")
            .and_then(|r| r.get("playerDieRolls"))
            .and_then(|r| r.as_array());

        match rolls {
            None => vec![],
            Some(rolls) => rolls
                .iter()
                .filter_map(|r| {
                    Some(GameEvent::DieRollResult {
                        seat_id: r.get("systemSeatId")?.as_u64()? as u8,
                        roll_value: r.get("rollValue")?.as_u64()? as u32,
                    })
                })
                .collect(),
        }
    }

}

/// Converts a flat card ID array (with duplicates) into a DeckLoaded event.
/// Used for both ConnectResp.deckMessage and SubmitDeckReq.deck.
fn flat_array_to_deck_loaded(deck: &Value) -> Option<GameEvent> {
    let raw_cards = deck.get("deckCards")?.as_array()?;
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for c in raw_cards {
        if let Some(id) = c.as_u64() {
            *counts.entry(id as u32).or_insert(0) += 1;
        }
    }

    let cards = counts
        .into_iter()
        .map(|(card_id, quantity)| DeckCard { card_id, quantity })
        .collect();

    let commander = deck
        .get("commanderCards")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.as_u64())
        .map(|id| id as u32);

    Some(GameEvent::DeckLoaded { cards, commander })
}

fn extract_int_detail(details: &[Value], key: &str) -> Option<i64> {
    details.iter().find_map(|d| {
        if d.get("key")?.as_str()? == key {
            d.get("valueInt32")?.as_array()?.first()?.as_i64()
        } else {
            None
        }
    })
}
