use crate::dlog;
use crate::events::GameEvent;
use crate::parser;
use crate::parser::gre_message::GreParser;
use crate::segmenter::Chunk;
use std::sync::mpsc::{Receiver, Sender};

pub fn start(rx: Receiver<Chunk>, tx: Sender<GameEvent>) {
    std::thread::spawn(move || {
        run(rx, tx);
    });
}

enum ChunkType {
    MatchRoomState,
    GreToClient,
    CourseData,
    Other,
}

fn classify(json: &str) -> ChunkType {
    if json.contains("matchGameRoomStateChangedEvent") {
        ChunkType::MatchRoomState
    } else if json.contains("greToClientEvent") {
        ChunkType::GreToClient
    } else if json.contains("\"Courses\"") {
        ChunkType::CourseData
    } else {
        ChunkType::Other
    }
}

/// Chunks may begin with a header line (e.g. timestamp + transaction info)
/// before the JSON body. Return the substring starting at the first `{`,
/// or None if there's no JSON in this chunk.
fn extract_json(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    Some(&content[start..])
}

/// Extract the local MTGA user_id from a chunk header line. Returns None if
/// the line doesn't match either direction marker.
fn extract_local_user_id(line: &str) -> Option<String> {
    // "... Match to USER_ID: ..."
    if let Some(idx) = line.find("Match to ") {
        let after = &line[idx + "Match to ".len()..];
        if let Some(end) = after.find(':') {
            let uid = after[..end].trim();
            if !uid.is_empty() {
                return Some(uid.to_string());
            }
        }
    }
    // "... USER_ID to Match: ..."
    if let Some(idx) = line.find(" to Match:") {
        let before = &line[..idx];
        if let Some(start) = before.rfind(' ') {
            let uid = before[start + 1..].trim();
            if !uid.is_empty() {
                return Some(uid.to_string());
            }
        }
    }
    None
}

fn run(rx: Receiver<Chunk>, tx: Sender<GameEvent>) {
    let mut gre = GreParser::new();
    let mut chunk_count: u64 = 0;
    let mut last_local_user: Option<String> = None;

    for chunk in rx {
        chunk_count += 1;

        // Identify the locally logged-in MTGA user from the header line. The
        // segmenter strips the `[UnityCrossThreadLogger]` prefix; what's left
        // on the first line is the timestamp + direction marker. Both
        // directions encode the local user_id:
        //   "... Match to USER_ID: <messageType>"   (server → client)
        //   "... USER_ID to Match: <messageType>"   (client → server)
        // We re-emit only when the user changes (login, account switch),
        // which is rare — typically once per app session.
        if let Some(first_line) = chunk.content.lines().next() {
            if let Some(uid) = extract_local_user_id(first_line) {
                if last_local_user.as_deref() != Some(&uid) {
                    last_local_user = Some(uid.clone());
                    if tx
                        .send(GameEvent::LocalPlayerIdentified { user_id: uid })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }

        let json = match extract_json(&chunk.content) {
            Some(j) => j,
            None => {
                if chunk_count % 100 == 0 {
                    dlog!("[chunk #{}] (no JSON; latest = Other)", chunk_count);
                }
                continue;
            }
        };

        let class = classify(json);
        let label = match class {
            ChunkType::MatchRoomState => "MatchRoomState",
            ChunkType::GreToClient => "GreToClient",
            ChunkType::CourseData => "CourseData",
            ChunkType::Other => "Other",
        };
        if !matches!(class, ChunkType::Other) {
            dlog!(
                "[chunk #{}] {} ({} bytes)",
                chunk_count,
                label,
                json.len()
            );
        } else if chunk_count % 100 == 0 {
            dlog!("[chunk #{}] (latest = Other)", chunk_count);
        }

        let events: Vec<GameEvent> = match class {
            ChunkType::MatchRoomState => {
                let events = parser::match_state::parse(json);
                if events.iter().any(|e| matches!(e, GameEvent::MatchStarted { .. })) {
                    gre.reset();
                }
                events
            }
            ChunkType::GreToClient => gre.parse(json),
            ChunkType::CourseData => parser::course_data::parse(json),
            ChunkType::Other => vec![],
        };

        for event in events {
            if tx.send(event).is_err() {
                return;
            }
        }
    }
}
