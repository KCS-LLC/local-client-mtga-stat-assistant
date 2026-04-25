import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Module-level cache shared across components — once we've looked a card up,
// we never need to ask the backend again for that grpId.
const nameCache = new Map<number, string>();

/**
 * Returns a stable Map<grpId, name> covering every grpId in `ids`.
 * Backfills missing entries from the backend on each ids change.
 * IDs not present in MTGA's local card database stay missing from the result.
 */
export function useCardNames(ids: Iterable<number>): Map<number, string> {
  const idArray = Array.from(new Set(ids));
  const idsKey = idArray.slice().sort((a, b) => a - b).join(",");

  const [, force] = useState(0);

  useEffect(() => {
    const missing = idArray.filter((id) => !nameCache.has(id));
    if (missing.length === 0) return;

    let cancelled = false;
    invoke<Record<string, string>>("get_card_names", { grpIds: missing })
      .then((result) => {
        if (cancelled) return;
        for (const [k, v] of Object.entries(result)) {
          nameCache.set(Number(k), v);
        }
        force((n) => n + 1);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idsKey]);

  const result = new Map<number, string>();
  for (const id of idArray) {
    const name = nameCache.get(id);
    if (name) result.set(id, name);
  }
  return result;
}
