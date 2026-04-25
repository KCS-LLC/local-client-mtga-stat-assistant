use crate::events::{GameEvent, MatchPlayer};
use serde_json::Value;

pub fn parse(content: &str) -> Vec<GameEvent> {
    try_parse(content).unwrap_or_default()
}

fn try_parse(content: &str) -> Option<Vec<GameEvent>> {
    let v: Value = serde_json::from_str(content).ok()?;
    let room_info = v
        .get("matchGameRoomStateChangedEvent")?
        .get("gameRoomInfo")?;

    let state_type = room_info.get("stateType")?.as_str()?;
    let timestamp = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(0);

    match state_type {
        "MatchGameRoomStateType_Playing" => parse_started(room_info, timestamp),
        "MatchGameRoomStateType_MatchCompleted" => parse_ended(room_info, timestamp),
        _ => Some(vec![]),
    }
}

fn parse_started(room_info: &Value, timestamp: u64) -> Option<Vec<GameEvent>> {
    let config = room_info.get("gameRoomConfig")?;
    let match_id = config.get("matchId")?.as_str()?.to_string();
    let players = config.get("reservedPlayers")?.as_array()?;

    if players.len() < 2 {
        return None;
    }

    let format = players[0]
        .get("eventId")
        .and_then(|e| e.as_str())
        .unwrap_or("Unknown")
        .to_string();

    Some(vec![GameEvent::MatchStarted {
        match_id,
        player1: extract_player(&players[0])?,
        player2: extract_player(&players[1])?,
        format,
        timestamp,
    }])
}

fn parse_ended(room_info: &Value, timestamp: u64) -> Option<Vec<GameEvent>> {
    let config = room_info.get("gameRoomConfig")?;
    let match_id = config.get("matchId")?.as_str()?.to_string();

    let result_list = room_info
        .get("finalMatchResult")?
        .get("resultList")?
        .as_array()?;

    // Use the Match-scope result (as opposed to individual Game-scope results)
    let match_result = result_list.iter().find(|r| {
        r.get("scope")
            .and_then(|s| s.as_str())
            .map(|s| s == "MatchScope_Match")
            .unwrap_or(false)
    })?;

    let winning_team_id = match_result.get("winningTeamId")?.as_u64()? as u8;
    let reason = match_result
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("Unknown")
        .to_string();

    Some(vec![GameEvent::MatchEnded {
        match_id,
        winning_team_id,
        reason,
        timestamp,
    }])
}

fn extract_player(v: &Value) -> Option<MatchPlayer> {
    Some(MatchPlayer {
        user_id: v.get("userId")?.as_str()?.to_string(),
        name: v.get("playerName")?.as_str()?.to_string(),
        seat_id: v.get("systemSeatId")?.as_u64()? as u8,
        team_id: v.get("teamId")?.as_u64()? as u8,
    })
}
