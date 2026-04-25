export type Zone =
  | "Library"
  | "Hand"
  | "Battlefield"
  | "Graveyard"
  | "Exile"
  | "Stack"
  | "Command"
  | "Limbo"
  | "Unknown";

export interface MatchPlayer {
  user_id: string;
  name: string;
  seat_id: number;
  team_id: number;
}

export interface DeckCard {
  card_id: number;
  quantity: number;
}

export type GameEvent =
  | {
      type: "MatchStarted";
      match_id: string;
      player1: MatchPlayer;
      player2: MatchPlayer;
      format: string;
      timestamp: number;
    }
  | {
      type: "PlayerIdentified";
      player_seat_id: number;
      opponent_seat_id: number;
      opponent: MatchPlayer;
    }
  | {
      type: "MatchEnded";
      match_id: string;
      winning_team_id: number;
      reason: string;
      timestamp: number;
    }
  | { type: "DeckLoaded"; cards: DeckCard[]; commander: number | null }
  | {
      type: "ZoneChanged";
      instance_id: number;
      card_id: number;
      from_zone: Zone;
      to_zone: Zone;
      owner_seat_id: number;
      face_down: boolean;
    }
  | {
      type: "DeckSnapshot";
      deck_id: string;
      deck_name: string;
      cards: DeckCard[];
    }
  | {
      type: "GameEnded";
      winning_team_id: number;
      game_number: number;
      sideboard_next: boolean;
    }
  | { type: "DieRollResult"; seat_id: number; roll_value: number }
  | { type: "CommanderRevealed"; card_id: number; seat_id: number }
  | {
      type: "CommanderCast";
      card_id: number;
      seat_id: number;
      cast_count: number;
      tax: number;
    }
  | { type: "CommanderReturned"; card_id: number; seat_id: number }
  | { type: "LibraryShuffle"; seat_id: number };
