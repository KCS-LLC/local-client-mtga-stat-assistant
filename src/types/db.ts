export interface DeckWL {
  deck_name: string;
  wins: number;
  losses: number;
}

export interface MatchRecord {
  match_id: string;
  format: string;
  opponent_name: string;
  deck_id: string | null;
  deck_name: string | null;
  result: string | null;
  won_die_roll: boolean | null;
  played_first: boolean | null;
  started_at: number;
}

export interface DeckSnapshot {
  deck_id: string;
  deck_name: string;
  cards: Record<string, number>;
}
