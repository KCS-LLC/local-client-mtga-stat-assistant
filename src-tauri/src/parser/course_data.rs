use crate::events::{DeckCard, GameEvent};
use serde_json::Value;

pub fn parse(content: &str) -> Vec<GameEvent> {
    try_parse(content).unwrap_or_default()
}

fn try_parse(content: &str) -> Option<Vec<GameEvent>> {
    let v: Value = serde_json::from_str(content).ok()?;
    let courses = v.get("Courses")?.as_array()?;

    let events = courses
        .iter()
        .filter_map(parse_course)
        .collect();

    Some(events)
}

fn parse_course(course: &Value) -> Option<GameEvent> {
    let summary = course.get("CourseDeckSummary")?;
    let deck_id = summary.get("DeckId")?.as_str()?.to_string();
    let deck_name = summary.get("Name")?.as_str()?.to_string();

    let deck = course.get("CourseDeck")?;
    let main = deck.get("MainDeck")?.as_array()?;
    let command = deck
        .get("CommandZone")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    let mut cards: Vec<DeckCard> = main
        .iter()
        .filter_map(parse_deck_card)
        .collect();

    // Include commander zone cards in the snapshot
    for card in command.iter().filter_map(parse_deck_card) {
        cards.push(card);
    }

    Some(GameEvent::DeckSnapshot {
        deck_id,
        deck_name,
        cards,
    })
}

fn parse_deck_card(v: &Value) -> Option<DeckCard> {
    Some(DeckCard {
        card_id: v.get("cardId")?.as_u64()? as u32,
        quantity: v.get("quantity")?.as_u64()? as u32,
    })
}
