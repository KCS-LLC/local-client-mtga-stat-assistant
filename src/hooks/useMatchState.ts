import { useEffect, useReducer } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { GameEvent, MatchPlayer } from "../types/events";

export interface MatchState {
  mtgaRunning: boolean;
  inMatch: boolean;
  matchId: string | null;
  format: string | null;
  opponent: MatchPlayer | null;
  playerSeatId: number | null;
  opponentSeatId: number | null;
  gameNumber: number;
  // game_number → (instance_id → grp_id) for unique cards we've seen the
  // opponent put on the Stack or Battlefield this game. Tracking by
  // instance_id dedupes the same physical card moving zones (cast + resolve
  // is two events but one instance). Display layer groups by grp_id and counts.
  opponentInstances: Map<number, Map<number, number>>;
  // Same shape, but the player's own cast/played cards.
  playerInstances: Map<number, Map<number, number>>;
  // seat_id → (grpId → current commander tax)
  commanderTax: Map<number, Map<number, number>>;
  // Player's own deck for this match: grp_id → quantity (includes commander)
  playerDeck: Map<number, number> | null;
  // Commander grpId for our deck (excluded from library because it lives in
  // the command zone, not the library)
  playerCommander: number | null;
  // Player's library state (decremented as cards leave Library)
  playerLibrary: Map<number, number> | null;
  playerLibrarySize: number;
  // grpId of the top card of player's library when known (after scry,
  // surveil, top-deck tutor, etc.). null when the top is uniformly random.
  playerKnownTop: number | null;
  // Bo3: per-game results so far this match (newest last)
  gameResults: Array<{ game_number: number; winning_team_id: number }>;
  // True between games of a Bo3 (sideboard window): GameEnded fired with
  // sideboard_next=true and the next game's state hasn't arrived yet.
  intermissionActive: boolean;
  // Diagnostic
  eventCount: number;
  lastEventType: string | null;
}

type Action =
  | { type: "mtga_status"; running: boolean }
  | { type: "game_event"; event: GameEvent };

function initial(): MatchState {
  return {
    mtgaRunning: false,
    inMatch: false,
    matchId: null,
    format: null,
    opponent: null,
    playerSeatId: null,
    opponentSeatId: null,
    gameNumber: 1,
    opponentInstances: new Map(),
    playerInstances: new Map(),
    commanderTax: new Map(),
    playerDeck: null,
    playerCommander: null,
    playerLibrary: null,
    playerLibrarySize: 0,
    playerKnownTop: null,
    gameResults: [],
    intermissionActive: false,
    eventCount: 0,
    lastEventType: null,
  };
}

function reducer(state: MatchState, action: Action): MatchState {
  if (action.type === "mtga_status") {
    return { ...state, mtgaRunning: action.running };
  }

  const e = action.event;
  // Count every event and track last type for diagnostics
  state = {
    ...state,
    eventCount: state.eventCount + 1,
    lastEventType: e.type,
  };
  switch (e.type) {
    case "MatchStarted":
      // Tentative — assume player1 = us. PlayerIdentified arrives next and
      // corrects this if the local player is actually player2 in this match.
      // NOTE: do NOT reset playerDeck / playerLibrary here. DeckLoaded fires
      // BEFORE MatchStarted (ConnectResp precedes matchGameRoomStateChange),
      // so by the time MatchStarted arrives, DeckLoaded has already set the
      // library for this match. Clearing it would wipe out fresh data.
      return {
        ...state,
        inMatch: true,
        matchId: e.match_id,
        format: e.format,
        opponent: e.player2,
        playerSeatId: e.player1.seat_id,
        opponentSeatId: e.player2.seat_id,
        gameNumber: 1,
        opponentInstances: new Map(),
        playerInstances: new Map(),
        commanderTax: new Map(),
        gameResults: [],
        intermissionActive: false,
      };

    case "PlayerIdentified":
      return {
        ...state,
        playerSeatId: e.player_seat_id,
        opponentSeatId: e.opponent_seat_id,
        opponent: e.opponent,
      };

    case "MatchEnded":
      return { ...state, inMatch: false, intermissionActive: false };

    case "GameEnded":
      return {
        ...state,
        gameNumber: e.game_number + 1,
        gameResults: [
          ...state.gameResults,
          {
            game_number: e.game_number,
            winning_team_id: e.winning_team_id,
          },
        ],
        // Open the sideboard-window summary only when more games follow.
        // Bo1 (Brawl, Standard ranked) and the final game of Bo3 set
        // sideboard_next=false, so no banner appears.
        intermissionActive: e.sideboard_next,
      };

    case "ZoneChanged": {
      let next = state;

      // Decrement player's library when a card leaves it (drawn, milled,
      // tutored, scryed away, etc.) — for our seat only. We don't track the
      // opponent's library because their card identities are hidden.
      if (
        e.from_zone === "Library" &&
        e.owner_seat_id === state.playerSeatId &&
        next.playerLibrary !== null
      ) {
        const lib = new Map(next.playerLibrary);
        const count = lib.get(e.card_id);
        if (count !== undefined) {
          if (count > 1) lib.set(e.card_id, count - 1);
          else lib.delete(e.card_id);
          next = {
            ...next,
            playerLibrary: lib,
            playerLibrarySize: Math.max(0, next.playerLibrarySize - 1),
          };
        }
      }

      // Track the first time we see a card hit Stack or Battlefield. Stack
      // covers cast spells that resolve to graveyard (Adventurous Impulse).
      // Battlefield covers lands and resolved permanents. First-write-wins
      // keeps the dedupe-by-instance semantics correct.
      if (
        (e.to_zone === "Battlefield" || e.to_zone === "Stack") &&
        !e.face_down
      ) {
        const isPlayer = e.owner_seat_id === state.playerSeatId;
        const isOpponent = e.owner_seat_id === state.opponentSeatId;
        if (!isPlayer && !isOpponent) return next;

        const target = isOpponent ? next.opponentInstances : next.playerInstances;
        const byGame = new Map(target);
        const instanceMap = new Map(byGame.get(state.gameNumber) ?? []);
        if (instanceMap.has(e.instance_id)) return next;
        instanceMap.set(e.instance_id, e.card_id);
        byGame.set(state.gameNumber, instanceMap);
        return isOpponent
          ? { ...next, opponentInstances: byGame }
          : { ...next, playerInstances: byGame };
      }
      return next;
    }

    case "DeckLoaded": {
      const deck = new Map<number, number>();
      for (const c of e.cards) {
        deck.set(c.card_id, (deck.get(c.card_id) ?? 0) + c.quantity);
      }
      if (e.commander != null) {
        deck.set(e.commander, (deck.get(e.commander) ?? 0) + 1);
      }
      // Library starts as a copy of the deck. Commanders sit in the Command
      // zone, not the library — exclude them from the initial library count.
      const library = new Map(deck);
      if (e.commander != null) {
        const c = library.get(e.commander) ?? 0;
        if (c > 1) library.set(e.commander, c - 1);
        else library.delete(e.commander);
      }
      const librarySize = Array.from(library.values()).reduce(
        (sum, q) => sum + q,
        0,
      );
      return {
        ...state,
        playerDeck: deck,
        playerCommander: e.commander,
        playerLibrary: library,
        playerLibrarySize: librarySize,
      };
    }

    case "ZoneStateSync": {
      // Initial-state snapshot from a Full GameStateMessage. Cards in the
      // opening hand never trigger a Library→Hand transition annotation, so
      // without this we'd never decrement them from the library. Recompute
      // library = deck − commander − every visible non-library card for our
      // seat. Idempotent: re-receiving the same snapshot produces the same
      // library, so this is safe to handle on every Full event (e.g. after
      // a reconnect mid-game).

      // First state from the new game also dismisses the Bo3 intermission
      // summary banner (the sideboard window is over).
      const dismissIntermission = state.intermissionActive;

      if (e.seat_id !== state.playerSeatId || state.playerDeck === null) {
        return dismissIntermission
          ? { ...state, intermissionActive: false }
          : state;
      }
      const lib = new Map<number, number>(state.playerDeck);
      // Pull the commander out of the library baseline
      if (state.playerCommander !== null) {
        const c = lib.get(state.playerCommander) ?? 0;
        if (c > 1) lib.set(state.playerCommander, c - 1);
        else lib.delete(state.playerCommander);
      }
      const seen = [
        ...e.hand,
        ...e.battlefield,
        ...e.graveyard,
        ...e.exile,
        ...e.stack,
      ];
      for (const id of seen) {
        const c = lib.get(id);
        if (c === undefined) continue;
        if (c > 1) lib.set(id, c - 1);
        else lib.delete(id);
      }
      const size = Array.from(lib.values()).reduce((s, q) => s + q, 0);
      return {
        ...state,
        playerLibrary: lib,
        playerLibrarySize: size,
        playerKnownTop: e.top_of_library ?? null,
        intermissionActive: dismissIntermission ? false : state.intermissionActive,
      };
    }

    case "CommanderRevealed": {
      const bySeat = new Map(state.commanderTax);
      const seatMap = new Map(bySeat.get(e.seat_id) ?? []);
      // Don't overwrite if already cast (would clobber a real tax value)
      if (!seatMap.has(e.card_id)) {
        seatMap.set(e.card_id, 0);
      }
      bySeat.set(e.seat_id, seatMap);
      return { ...state, commanderTax: bySeat };
    }

    case "CommanderCast": {
      const bySeat = new Map(state.commanderTax);
      const seatMap = new Map(bySeat.get(e.seat_id) ?? []);
      seatMap.set(e.card_id, e.tax);
      bySeat.set(e.seat_id, seatMap);
      return { ...state, commanderTax: bySeat };
    }

    default:
      return state;
  }
}

export function useMatchState(): MatchState {
  const [state, dispatch] = useReducer(reducer, undefined, initial);

  useEffect(() => {
    const unlistenStatus = listen<boolean>("mtga_status", (e) => {
      console.log("[mtga_status]", e.payload);
      dispatch({ type: "mtga_status", running: e.payload });
    });

    const unlistenEvent = listen<GameEvent>("game_event", (e) => {
      console.log("[game_event]", e.payload);
      dispatch({ type: "game_event", event: e.payload });
    });

    // Bootstrap initial status (in case the backend pushed before we listened)
    invoke<boolean>("get_mtga_status")
      .then((running) => dispatch({ type: "mtga_status", running }))
      .catch(() => {});

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenEvent.then((fn) => fn());
    };
  }, []);

  return state;
}
