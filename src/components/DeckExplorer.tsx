import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeckSnapshot } from "../types/db";
import { useCardInfo, type CardInfo } from "../hooks/useCardNames";

interface Props {
  /** Optional deck_id to pre-select (e.g. from a click in match history) */
  initialDeckId?: string | null;
}

function cardLabel(id: number, info: Map<number, CardInfo>): string {
  return info.get(id)?.name ?? `Card #${id}`;
}

/**
 * Sort entries [grpId, qty] by CMC ascending, lands at the bottom
 * (alphabetical within lands), unresolved cards sort last within each group.
 * Matches MTGA's native deck view order.
 */
function sortByCmcLandsBottom(
  entries: Array<[number, number]>,
  info: Map<number, CardInfo>,
): Array<[number, number]> {
  return entries.slice().sort((a, b) => {
    const ai = info.get(a[0]);
    const bi = info.get(b[0]);
    const aLand = ai?.is_land ? 1 : 0;
    const bLand = bi?.is_land ? 1 : 0;
    if (aLand !== bLand) return aLand - bLand;
    // Within non-lands: CMC ascending; within lands: alphabetical only
    if (!aLand) {
      const aCmc = ai?.cmc ?? 999;
      const bCmc = bi?.cmc ?? 999;
      if (aCmc !== bCmc) return aCmc - bCmc;
    }
    const an = ai?.name ?? `~~~${a[0]}`;
    const bn = bi?.name ?? `~~~${b[0]}`;
    return an.localeCompare(bn);
  });
}

export function DeckExplorer({ initialDeckId }: Props) {
  const [decks, setDecks] = useState<DeckSnapshot[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(
    initialDeckId ?? null,
  );
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    invoke<DeckSnapshot[]>("get_decks")
      .then((d) => {
        setDecks(d);
        setLoaded(true);
        // If no preselection, auto-pick the first deck so the right pane
        // isn't empty on first open
        setSelectedId((current) => current ?? d[0]?.deck_id ?? null);
      })
      .catch(() => setLoaded(true));
  }, []);

  // Track changes to initialDeckId (e.g., user clicked a different deck
  // name in match history) and re-select.
  useEffect(() => {
    if (initialDeckId) setSelectedId(initialDeckId);
  }, [initialDeckId]);

  const selected = decks.find((d) => d.deck_id === selectedId) ?? null;

  // Collect every grpId we need names for (the selected deck only — no point
  // resolving names for decks we aren't showing)
  const allIds = useMemo(() => {
    const set = new Set<number>();
    if (selected) {
      for (const k of Object.keys(selected.cards)) set.add(Number(k));
    }
    return set;
  }, [selected]);
  const info = useCardInfo(allIds);

  const entries: Array<[number, number]> = selected
    ? Object.entries(selected.cards).map(([k, v]) => [Number(k), v])
    : [];
  const sorted = sortByCmcLandsBottom(entries, info);
  const totalCards = entries.reduce((sum, [, q]) => sum + q, 0);

  if (loaded && decks.length === 0) {
    return (
      <div className="p-6 max-w-5xl mx-auto">
        <p className="text-sm text-zinc-500">
          No decks captured yet. They'll appear here once MTGA writes a deck
          list to the log (open the Decks screen in MTGA to trigger a refresh).
        </p>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-5xl mx-auto">
      <div className="grid grid-cols-3 gap-6">
        <aside className="col-span-1 border-r border-zinc-200 dark:border-zinc-800 pr-4">
          <h3 className="text-sm font-medium mb-2 text-zinc-500">
            Saved decks ({decks.length})
          </h3>
          <ul className="space-y-1 text-sm max-h-[70vh] overflow-auto">
            {decks.map((d) => {
              const active = d.deck_id === selectedId;
              return (
                <li key={d.deck_id}>
                  <button
                    type="button"
                    onClick={() => setSelectedId(d.deck_id)}
                    className={`w-full text-left px-2 py-1 rounded ${
                      active
                        ? "bg-zinc-200 dark:bg-zinc-800 font-medium"
                        : "hover:bg-zinc-100 dark:hover:bg-zinc-900"
                    }`}
                  >
                    {d.deck_name}
                  </button>
                </li>
              );
            })}
          </ul>
        </aside>

        <section className="col-span-2">
          {selected ? (
            <>
              <div className="mb-3 flex items-baseline justify-between">
                <h2 className="text-lg font-semibold">{selected.deck_name}</h2>
                <span className="text-xs text-zinc-500">
                  {totalCards} cards · {entries.length} unique
                </span>
              </div>
              <ul className="space-y-1 text-sm max-h-[70vh] overflow-auto">
                {sorted.map(([id, qty]) => {
                  const isLand = info.get(id)?.is_land === true;
                  return (
                    <li
                      key={id}
                      className="flex justify-between gap-2 text-zinc-700 dark:text-zinc-300"
                    >
                      <span
                        className={
                          isLand
                            ? "text-emerald-700 dark:text-emerald-400"
                            : ""
                        }
                      >
                        {cardLabel(id, info)}
                      </span>
                      <span className="text-zinc-500 tabular-nums whitespace-nowrap">
                        × {qty}
                      </span>
                    </li>
                  );
                })}
              </ul>
            </>
          ) : (
            <p className="text-sm text-zinc-500">Select a deck on the left.</p>
          )}
        </section>
      </div>
    </div>
  );
}
