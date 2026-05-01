# MTGA Stat Assistant

A lightweight desktop companion for **Magic: The Gathering Arena** that runs alongside the game — designed for a second monitor. Built with Tauri (Rust + React), so it uses minimal CPU and memory.

Reads MTGA's local log file in real time. No network calls, no account required, no data leaves your machine (except Scryfall API requests for card hover images).

> **Windows and macOS.** Linux is not supported (no native MTGA client).

---

## Features

**Match tracking**
- Win/loss record per deck, with overall win rate
- Full match history — opponent name, format, deck, date, result
- Play-order tracking: win rate when going first vs second
- Automatic deck correlation — matches are linked to the deck you registered

**In-game overlay** (second monitor)
- Live library view with per-card draw probability
- Known top-of-library detection (scry, surveil, tutor effects)
- Land count and P(land on next draw), updating as cards are drawn
- Cards you've played this game
- Cards your opponent has revealed
- Commander tax tracker (Commander / Brawl formats)
- Bo3 intermission banner with current game score

**Deck browser**
- Snapshot of all your current MTGA decks
- Sort by mana cost or alphabetically, lands at bottom
- Mana cost column with full symbol notation ({2}{W} etc.)

**Card images**
- Hover any card name anywhere in the app to see its art
- Toggle between Scryfall and Gatherer as the image source (Settings)
- Arena-only rebalanced cards (A- prefix) fall back gracefully

**Data**
- Export full match history to JSON (Save As dialog)
- SQLite database stored locally at `%APPDATA%\local-client-mtga-stat-assistant\`
- Auto-backup of the database on user switch

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.75+)
- [Node.js](https://nodejs.org/) (18+)
- [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) — on Windows this means the Visual Studio C++ build tools and WebView2 (already installed on Windows 11)
- MTGA installed with **Detailed Logs** enabled:
  `Options → View Account → Detailed Logs (Plugin Support) → ON`

---

## Installing a release build

Download the latest release from the [Releases page](../../releases):

- **Windows:** run the `.msi` installer. SmartScreen may warn about an unknown publisher — click "More info" → "Run anyway". This is normal for unsigned community software.
- **macOS:** open the `.dmg`, drag the app to Applications. On first launch, right-click → Open (required once for apps from outside the App Store).

## Building from source

```bash
git clone https://github.com/KCS-LLC/local-client-mtga-stat-assistant.git
cd local-client-mtga-stat-assistant
npm install
npm run tauri dev      # development build with hot reload
npm run tauri build    # production build + installer
```

The production installer is written to `src-tauri/target/release/bundle/`.

---

## Log file location

The app auto-detects the log — no configuration needed.

| Platform | Path |
|---|---|
| Windows | `%AppData%\..\LocalLow\Wizards Of The Coast\MTGA\Player.log` |
| macOS | `~/Library/Logs/Wizards Of The Coast/MTGA/Player.log` |

---

## MTGA card database

Card names, mana costs, and CMC are loaded from MTGA's local SQLite database:

```
<MTGA install drive>:\Program Files\Wizards of the Coast\MTGA\MTGA_Data\Downloads\Raw\Raw_CardDatabase_*.mtga
```

| Platform | Path |
|---|---|
| Windows | `<drive>:\Program Files\Wizards of the Coast\MTGA\MTGA_Data\Downloads\Raw\` (scans C–Z) |
| macOS | `~/Library/Application Support/com.wizards.mtga/Downloads/Data/` |

If the card DB isn't found, card IDs display as `Card #12345` instead of names — everything else still works.

---

## Data & privacy

- All data stays on your machine.
- The only outbound requests are to the [Scryfall API](https://scryfall.com/docs/api) for card hover images, and only when you hover a card name. You can switch to Gatherer (which uses multiverse IDs from Scryfall's data) or disable hover images by not hovering.
- No telemetry, no analytics, no accounts.

---

## License

MIT
