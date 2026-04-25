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

---

## Status

| Module | Status |
|---|---|
| Project scaffold | Planned |
| Log file parser (Rust) | Planned |
| Match start/end events | Planned |
| Deck list extraction | Planned |
| Zone change tracking | Planned |
| Win/loss tracker UI | Planned |
| Opponent card log UI | Planned |
| Draw odds calculator | Planned |
| Draw odds UI | Planned |
| Persistent match history | Planned |
| Settings / configuration | Planned |

---

## Tech Stack

| Layer | Technology |
|---|---|
| Backend | Rust |
| Desktop shell | Tauri 2.x |
| Frontend | HTML / CSS / TypeScript |
| Log watching | Rust (`notify` crate) |
| Probability math | Rust (hypergeometric distribution) |
| Local storage | SQLite via `rusqlite` |
| Frontend framework | TBD |

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

## Contributing

Contributions are welcome. Please open an issue before starting work on a significant feature so we can discuss approach and avoid duplicate effort.

---

## License

MIT
