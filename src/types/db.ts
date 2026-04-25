export interface DeckWL {
  deck_name: string;
  wins: number;
  losses: number;
}

export interface MatchRecord {
  match_id: string;
  format: string;
  opponent_name: string;
  deck_name: string | null;
  result: string | null;
  won_die_roll: boolean | null;
  played_first: boolean | null;
  started_at: number;
}
