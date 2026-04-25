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

fn classify(content: &str) -> ChunkType {
    if !content.starts_with('{') {
        return ChunkType::Other;
    }
    if content.contains("matchGameRoomStateChangedEvent") {
        ChunkType::MatchRoomState
    } else if content.contains("greToClientEvent") {
        ChunkType::GreToClient
    } else if content.contains("\"Courses\"") {
        ChunkType::CourseData
    } else {
        ChunkType::Other
    }
}

fn run(rx: Receiver<Chunk>, tx: Sender<GameEvent>) {
    let mut gre = GreParser::new();

    for chunk in rx {
        let events: Vec<GameEvent> = match classify(&chunk.content) {
            ChunkType::MatchRoomState => {
                let events = parser::match_state::parse(&chunk.content);
                // Reset GRE parser state on each new match
                if events.iter().any(|e| matches!(e, GameEvent::MatchStarted { .. })) {
                    gre.reset();
                }
                events
            }
            ChunkType::GreToClient => gre.parse(&chunk.content),
            ChunkType::CourseData => parser::course_data::parse(&chunk.content),
            ChunkType::Other => vec![],
        };

        for event in events {
            if tx.send(event).is_err() {
                return;
            }
        }
    }
}
