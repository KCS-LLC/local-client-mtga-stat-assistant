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
  // Player's library state (decremented as cards leave Library)
  playerLibrary: Map<number, number> | null;
  playerLibrarySize: number;
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
    playerLibrary: null,
    playerLibrarySize: 0,
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
        playerDeck: null,
        playerLibrary: null,
        playerLibrarySize: 0,
      };

    case "PlayerIdentified":
      return {
        ...state,
        playerSeatId: e.player_seat_id,
        opponentSeatId: e.opponent_seat_id,
        opponent: e.opponent,
      };

    case "MatchEnded":
      return { ...state, inMatch: false };

    case "GameEnded":
      return { ...state, gameNumber: e.game_number + 1 };

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
        playerLibrary: library,
        playerLibrarySize: librarySize,
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
