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

fn run(rx: Receiver<Chunk>, tx: Sender<GameEvent>) {
    let mut gre = GreParser::new();
    let mut chunk_count: u64 = 0;

    for chunk in rx {
        chunk_count += 1;

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
