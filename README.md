# Local Client MTGA Stat Assistant

A lightweight desktop companion for **Magic: The Gathering Arena** that runs alongside the game on a second monitor. Built with Tauri (Rust + WebView2) for minimal resource usage.

Reads MTGA's local log file in real time — no network calls, no data uploads, no third-party accounts required.

---

## Planned Features

### Match Tracking
- [ ] Win/loss record tracked per deck
- [ ] Match history with opponent name, format, date, and result
- [ ] Win rate displayed per deck over configurable time windows (session / 7d / 30d / all-time)
- [ ] Persistent storage of match history across sessions
- [ ] Play/draw tracking — record whether you played first or drew first per match (on by default)
- [ ] Win rate on the play vs on the draw
- [ ] Coin flip / die roll result tracking per match (on by default)
- [ ] Win rate when winning vs losing the flip/roll

### Opponent Tracking
- [ ] Cards played by opponent logged per match (battlefield entries only — hidden hand/library not accessible)
- [ ] Per-match opponent card history viewable after game ends
- [ ] Opponent name and avatar/deck commander recorded per match

### Real-Time Draw Odds
- [ ] Starting deck list loaded automatically from log at match start
- [ ] Library size tracked in real time as cards are drawn
- [ ] Probability of drawing a land on next draw (hypergeometric distribution)
- [ ] Probability of drawing a specific card (or any copy of it) within next N draws
- [ ] Configurable "watch list" of cards to track odds for during a game

### General
- [ ] Separate window — designed for a second monitor, not an overlay
- [ ] Requires "Detailed Logs (Plugin Support)" enabled in MTGA settings (one-time setup)
- [ ] Windows support (primary)
- [ ] macOS support (planned)
- [ ] No MTGA account login required — local log only
- [ ] Export full match history to JSON

### Deck History
- [ ] Deck list snapshot saved per session (off by default, user-configurable)
  - MTGA reports all current deck lists on every launch — this feature captures and stores those snapshots over time
  - Useful for tracking how decks evolve, and as input for the future aggregation project

### Data & Storage
- [ ] SQLite database for persistent match history (zero install overhead — bundled in app binary)
- [ ] Automatic `.bak` file created on each app launch (on by default, user-configurable)
- [ ] JSON export for portability and use with external analysis tools

---

## Status

| Module | Status |
|---|---|
| Project scaffold | Complete |
| Log file tailer (Rust, 250ms poll) | Complete |
| Log segmenter | Complete |
| Router (chunk classifier) | Complete |
| Match start/end events | Complete |
| Deck list extraction (ConnectResp + SubmitDeckReq) | Complete |
| Zone change tracking | Complete |
| Commander tax tracking | Complete |
| Commander return tracking (SBA) | Complete |
| Shuffle / instance ID invalidation | Complete |
| Bo3 game boundary events (IntermissionReq) | Complete |
| Die roll result events | Complete |
| Deck snapshot extraction (CourseData) | Complete |
| SQLite DB layer | Complete |
| Match/game/opponent-card persistence | Complete |
| W/L stats + match history Tauri commands | Complete |
| Win/loss tracker UI | Not started |
| Opponent card log UI | Not started |
| Draw odds calculator (Rust) | Not started |
| Draw odds UI | Not started |
| Settings / configuration UI | Not started |
| DB backup on launch | Not started |
| JSON export | Not started |

---

## Tech Stack

| Layer | Technology |
|---|---|
| Backend | Rust |
| Desktop shell | Tauri 2.x |
| Frontend | HTML / CSS / TypeScript |
| Log watching | Rust (polling, 250ms interval) |
| Probability math | Rust (hypergeometric distribution) |
| Local storage | SQLite via `rusqlite` |
| Frontend framework | React 19 + TypeScript + Tailwind CSS v4 |

---

## Prerequisites

To run from source you will need:

- [Rust](https://rustup.rs/) (1.75+)
- [Node.js](https://nodejs.org/) (18+)
- [Tauri prerequisites for your platform](https://tauri.app/start/prerequisites/)
- MTG Arena installed with **Detailed Logs (Plugin Support)** enabled
  - In MTGA: `Options → Account → Detailed Logs (Plugin Support) → ON`

---

## Getting Started

> Setup instructions will be added once the initial scaffold is complete.

---

## Log File Location

| Platform | Path |
|---|---|
| Windows | `%AppData%\..\LocalLow\Wizards Of The Coast\MTGA\Player.log` |
| macOS | `~/Library/Logs/Wizards Of The Coast/MTGA/Player.log` |

---

## Architecture Decisions

| Decision | Choice | Reason |
|---|---|---|
| Frontend → backend comms | Tauri Events (push model) | Log stream is push by nature — Rust emits, React listens |
| Storage | SQLite via `rusqlite` | Zero user-facing install, handles years of match data at ~2-3MB/year |
| DB backup | Single `.bak` on launch, on by default | Corruption recovery without fragmenting history |
| DB rotation | Not implemented — unnecessary at expected data volumes | See size analysis below |
| Aggregation | Separate future project | Local client stays offline and lightweight; aggregation is opt-in |

### Settings table

Stored as key/value rows in SQLite. Defaults seeded on first launch:

```
settings
├── player_id             (string)  — MTGA userId; auto-detected from first match, user-editable
├── backup_on_launch      (boolean) — default: true
├── track_deck_history    (boolean) — default: false
├── track_play_draw       (boolean) — default: true
├── track_flip_roll       (boolean) — default: true
```

DB location: `%APPDATA%\local-client-mtga-stat-assistant\stats.db`

### Database schema

```
matches         — one row per match (format, players, deck, result, die roll, play/draw)
games           — one row per game within a match (game_number, winning_team_id)
opponent_cards  — one row per unique card seen on opponent's battlefield per game
settings        — key/value configuration
```

### Why not rotate the database by date or match count?

At ~2-3MB per year of serious play, the database will never approach a size that causes performance issues. Rotation would fragment match history and complicate cross-period queries. Instead, the app keeps one rolling `.bak` file for corruption recovery and provides a JSON export for users who want to archive or share their data.

---

## Contributing

Contributions are welcome. Please open an issue before starting work on a significant feature so we can discuss approach and avoid duplicate effort.

---

## License

MIT
