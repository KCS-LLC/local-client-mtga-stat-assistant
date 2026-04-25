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
  // Cards seen on opponent's battlefield, grouped by game number
  opponentCards: Map<number, Set<number>>;
  // seat_id → (grpId → current commander tax)
  commanderTax: Map<number, Map<number, number>>;
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
    opponentCards: new Map(),
    commanderTax: new Map(),
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
      // Frontend doesn't know which player is "us" — backend's event_sink
      // identifies via settings.player_id. For UI display, we just show
      // both players; opponent is whichever one isn't us. Until we wire
      // a settings query, default opponent = player2.
      return {
        ...state,
        inMatch: true,
        matchId: e.match_id,
        format: e.format,
        opponent: e.player2,
        playerSeatId: e.player1.seat_id,
        opponentSeatId: e.player2.seat_id,
        gameNumber: 1,
        opponentCards: new Map(),
        commanderTax: new Map(),
      };

    case "MatchEnded":
      return { ...state, inMatch: false };

    case "GameEnded":
      return { ...state, gameNumber: e.game_number + 1 };

    case "ZoneChanged": {
      if (
        e.to_zone === "Battlefield" &&
        e.owner_seat_id === state.opponentSeatId &&
        !e.face_down
      ) {
        const map = new Map(state.opponentCards);
        const set = new Set(map.get(state.gameNumber) ?? []);
        set.add(e.card_id);
        map.set(state.gameNumber, set);
        return { ...state, opponentCards: map };
      }
      return state;
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
