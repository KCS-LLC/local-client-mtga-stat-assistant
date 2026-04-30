import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeckSnapshot } from "../types/db";
import { useCardInfo, type CardInfo } from "../hooks/useCardNames";
import { CardName } from "./CardName";

type SortMode = "alpha" | "cmc";

const SORT_LABELS: Record<SortMode, string> = {
  alpha: "A–Z",
  cmc: "CMC",
};

// All modes in display order; extend here when Color / Type are added.
const SORT_MODES: SortMode[] = ["alpha", "cmc"];

interface Props {
  /** Optional deck_id to pre-select (e.g. from a click in match history) */
  initialDeckId?: string | null;
}

function cardLabel(id: number, info: Map<number, CardInfo>): string {
  return info.get(id)?.name ?? `Card #${id}`;
}

function sortEntries(
  mode: SortMode,
  entries: Array<[number, number]>,
  info: Map<number, CardInfo>,
): Array<[number, number]> {
  return entries.slice().sort((a, b) => {
    const ai = info.get(a[0]);
    const bi = info.get(b[0]);
    // Lands always at the bottom regardless of sort mode
    const aLand = ai?.is_land ? 1 : 0;
    const bLand = bi?.is_land ? 1 : 0;
    if (aLand !== bLand) return aLand - bLand;

    if (mode === "cmc" && !aLand) {
      const aCmc = ai?.cmc ?? 999;
      const bCmc = bi?.cmc ?? 999;
      if (aCmc !== bCmc) return aCmc - bCmc;
    }

    // Alphabetical tiebreak (and the only criterion for "alpha" mode)
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
  const [sortMode, setSortModeState] = useState<SortMode>(
    () => (localStorage.getItem("deckSortMode") as SortMode | null) ?? "cmc",
  );

  function setSortMode(mode: SortMode) {
    localStorage.setItem("deckSortMode", mode);
    setSortModeState(mode);
  }

  useEffect(() => {
    invoke<DeckSnapshot[]>("get_decks")
      .then((d) => {
        setDecks(d);
        setLoaded(true);
        setSelectedId((current) => current ?? d[0]?.deck_id ?? null);
      })
      .catch(() => setLoaded(true));
  }, []);

  useEffect(() => {
    if (initialDeckId) setSelectedId(initialDeckId);
  }, [initialDeckId]);

  const selected = decks.find((d) => d.deck_id === selectedId) ?? null;

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
  const sorted = sortEntries(sortMode, entries, info);
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
              <div className="mb-3 flex items-center justify-between gap-2">
                <h2 className="text-lg font-semibold truncate">{selected.deck_name}</h2>
                <div className="flex items-center gap-3 shrink-0">
                  <span className="text-xs text-zinc-500">
                    {totalCards} cards · {entries.length} unique
                  </span>
                  <div className="flex rounded border border-zinc-200 dark:border-zinc-700 overflow-hidden text-xs">
                    {SORT_MODES.map((mode) => (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => setSortMode(mode)}
                        className={`px-2 py-0.5 ${
                          sortMode === mode
                            ? "bg-zinc-200 dark:bg-zinc-700 font-medium"
                            : "hover:bg-zinc-100 dark:hover:bg-zinc-800 text-zinc-500"
                        }`}
                      >
                        {SORT_LABELS[mode]}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
              <ul className="space-y-1 text-sm max-h-[70vh] overflow-auto">
                {sorted.map(([id, qty]) => {
                  const cardInfo = info.get(id);
                  const isLand = cardInfo?.is_land === true;
                  const manaCost = cardInfo?.mana_cost ?? null;
                  return (
                    <li
                      key={id}
                      className="flex items-center gap-2 text-zinc-700 dark:text-zinc-300"
                    >
                      <span className="text-zinc-400 dark:text-zinc-500 text-xs whitespace-nowrap shrink-0 w-20">
                        {manaCost ?? ""}
                      </span>
                      <CardName
                        name={cardLabel(id, info)}
                        className={`flex-1 min-w-0 truncate ${isLand ? "text-emerald-700 dark:text-emerald-400" : ""}`}
                      />
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
