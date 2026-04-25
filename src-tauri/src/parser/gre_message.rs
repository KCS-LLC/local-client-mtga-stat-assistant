use crate::events::{DeckCard, GameEvent, Zone};
use serde_json::Value;
use std::collections::HashMap;

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
}

impl GreParser {
    pub fn new() -> Self {
        Self {
            zone_map: HashMap::new(),
            instance_map: HashMap::new(),
            visibility_map: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        self.zone_map.clear();
        self.instance_map.clear();
        self.visibility_map.clear();
    }

    pub fn parse(&mut self, content: &str) -> Vec<GameEvent> {
        let v: Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

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
                "GREMessageType_GameStateMessage" => {
                    events.extend(self.parse_game_state(msg));
                }
                _ => {}
            }
        }
        events
    }

    fn parse_connect_resp(&self, msg: &Value) -> Option<GameEvent> {
        let deck_msg = msg.get("connectResp")?.get("deckMessage")?;

        // deckCards is a flat array with duplicates representing multiple copies
        let raw_cards = deck_msg.get("deckCards")?.as_array()?;
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

        let commander = deck_msg
            .get("commanderCards")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.as_u64())
            .map(|id| id as u32);

        Some(GameEvent::DeckLoaded { cards, commander })
    }

    fn parse_game_state(&mut self, msg: &Value) -> Vec<GameEvent> {
        let gs = match msg.get("gameStateMessage") {
            Some(gs) => gs,
            None => return vec![],
        };

        let update_type = gs.get("type").and_then(|t| t.as_str()).unwrap_or("");

        // Full snapshot: rebuild zone and instance maps from scratch
        if update_type == "GameStateType_Full" {
            self.rebuild_maps(gs);
            return vec![];
        }

        // Diff: update maps with any new/changed objects and zones, then process annotations
        self.update_maps(gs);
        self.process_zone_transfers(gs)
    }

    fn rebuild_maps(&mut self, gs: &Value) {
        self.zone_map.clear();
        self.instance_map.clear();
        self.visibility_map.clear();

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
            if let Some(grp_id) = obj.get("grpId").and_then(|g| g.as_u64()) {
                self.instance_map.insert(instance_id as u32, grp_id as u32);
            }
            let vis = obj
                .get("visibility")
                .and_then(|v| v.as_str())
                .unwrap_or("Visibility_Public")
                .to_string();
            self.visibility_map.insert(instance_id as u32, vis);
        }
    }

    fn process_zone_transfers(&self, gs: &Value) -> Vec<GameEvent> {
        let annotations = match gs.get("annotations").and_then(|a| a.as_array()) {
            Some(a) => a,
            None => return vec![],
        };

        annotations
            .iter()
            .filter_map(|ann| self.try_zone_transfer(ann))
            .collect()
    }

    fn try_zone_transfer(&self, ann: &Value) -> Option<GameEvent> {
        // Only handle ZoneTransfer annotations
        let types = ann.get("type")?.as_array()?;
        let is_zone_transfer = types
            .iter()
            .any(|t| t.as_str() == Some("AnnotationType_ZoneTransfer"));
        if !is_zone_transfer {
            return None;
        }

        let instance_id = ann.get("affectedIds")?.as_array()?.first()?.as_u64()? as u32;
        let details = ann.get("details")?.as_array()?;

        let src_zone_id = extract_int_detail(details, "zone_src")? as u32;
        let dst_zone_id = extract_int_detail(details, "zone_dest")? as u32;

        let src_info = self.zone_map.get(&src_zone_id)?;
        let dst_info = self.zone_map.get(&dst_zone_id)?;

        let card_id = *self.instance_map.get(&instance_id)?;
        let owner_seat_id = src_info.owner_seat_id;

        let from_zone = Zone::from_str(&src_info.zone_type);
        let to_zone = Zone::from_str(&dst_info.zone_type);

        // Face-down: card going to exile with non-public visibility
        let face_down = matches!(to_zone, Zone::Exile)
            && self
                .visibility_map
                .get(&instance_id)
                .map(|v| v != "Visibility_Public")
                .unwrap_or(false);

        Some(GameEvent::ZoneChanged {
            card_id,
            from_zone,
            to_zone,
            owner_seat_id,
            face_down,
        })
    }
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
