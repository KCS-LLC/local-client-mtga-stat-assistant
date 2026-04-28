import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface CardInfo {
  name: string;
  is_token: boolean;
  is_land: boolean;
  cmc: number;
}

// Module-level cache shared across components — once we've looked a card up,
// we never need to ask the backend again for that grpId. Cleared on hot card
// database reloads is not currently wired up; if MTGA updates mid-session and
// we need fresh data, restart the app for now.
const cache = new Map<number, CardInfo>();

/**
 * Returns a Map<grpId, CardInfo> covering every grpId in `ids` that the
 * backend knows about. IDs absent from MTGA's local card database stay
 * missing from the result. Backfills on each ids change.
 */
export function useCardInfo(ids: Iterable<number>): Map<number, CardInfo> {
  const idArray = Array.from(new Set(ids));
  const idsKey = idArray.slice().sort((a, b) => a - b).join(",");

  const [, force] = useState(0);

  useEffect(() => {
    const missing = idArray.filter((id) => !cache.has(id));
    if (missing.length === 0) return;

    let cancelled = false;
    invoke<Record<string, CardInfo>>("get_card_info", { grpIds: missing })
      .then((result) => {
        if (cancelled) return;
        for (const [k, v] of Object.entries(result)) {
          cache.set(Number(k), v);
        }
        force((n) => n + 1);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idsKey]);

  const result = new Map<number, CardInfo>();
  for (const id of idArray) {
    const info = cache.get(id);
    if (info) result.set(id, info);
  }
  return result;
}
