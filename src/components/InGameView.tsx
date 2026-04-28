import { useReducer } from "react";
import type { MatchState } from "../hooks/useMatchState";
import { useCardInfo, type CardInfo } from "../hooks/useCardNames";

interface Props {
  state: MatchState;
}

/** Bo3 mid-match summary. Visible during the sideboard window between
 * games of a Bo3 (GameEnded fired with sideboard_next=true and the next
 * game's state hasn't arrived yet). The reducer auto-dismisses when the
 * new game's first ZoneStateSync lands; the × on this banner lets the
 * user dismiss earlier if they prefer. */
function IntermissionBanner({ state }: { state: MatchState }) {
  // Local "manually dismissed" flag — survives until next state change of
  // the underlying intermissionActive flag. Resets implicitly when the
  // banner's parent decides not to render it (intermissionActive=false).
  const [dismissed, dismiss] = useReducer(() => true, false);
  if (!state.intermissionActive || dismissed) return null;

  // Derive player's team from any game result (we always log the
  // perspective via player_team_id on the backend, but it's not exposed
  // on MatchState). Cheaper: look at most recent game vs. opponent seat.
  // We treat a game as "you won" iff the winning team is NOT the opponent's
  // (which works because games are 1v1 in MTGA's modes that go to 3 games).
  const games = state.gameResults;
  const wins = games.filter(
    (g) => g.winning_team_id !== state.opponentSeatId,
  ).length;
  const losses = games.length - wins;

  return (
    <div className="border-b border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-950 px-4 py-3 flex items-start gap-3">
      <div className="flex-1">
        <div className="text-sm font-semibold text-amber-800 dark:text-amber-300">
          Sideboarding for game {state.gameNumber} · You {wins}–{losses}{" "}
          {state.opponent?.name ?? "Opponent"}
        </div>
        <ul className="mt-1 text-xs text-amber-800 dark:text-amber-400 space-y-0.5">
          {games.map((g) => {
            const youWon = g.winning_team_id !== state.opponentSeatId;
            return (
              <li key={g.game_number}>
                Game {g.game_number}: {youWon ? "You won" : "You lost"}
              </li>
            );
          })}
        </ul>
      </div>
      <button
        type="button"
        onClick={dismiss}
        title="Dismiss"
        className="text-amber-700 dark:text-amber-400 hover:text-amber-900 dark:hover:text-amber-200 px-1 leading-none text-lg"
      >
        ×
      </button>
    </div>
  );
}

function cardLabel(id: number, info: Map<number, CardInfo>): string {
  return info.get(id)?.name ?? `Card #${id}`;
}

/** Sort entries [grpId, count] by CMC ascending, lands at bottom; alphabetical within same CMC. */
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
    if (!aLand) {
      const aCmc = ai?.cmc ?? 999;
      const bCmc = bi?.cmc ?? 999;
      if (aCmc !== bCmc) return aCmc - bCmc;
    }
    const an = ai?.name ?? `Card #${a[0]}`;
    const bn = info.get(b[0])?.name ?? `Card #${b[0]}`;
    return an.localeCompare(bn);
  });
}

function CommanderList({
  tax,
  info,
}: {
  tax: Map<number, number> | undefined;
  info: Map<number, CardInfo>;
}) {
  if (!tax || tax.size === 0) return null;
  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium mb-2">Commander</h3>
      <ul className="text-sm space-y-1">
        {Array.from(tax.entries()).map(([id, t]) => (
          <li key={id} className="flex justify-between">
            <span className="text-zinc-700 dark:text-zinc-300">
              {cardLabel(id, info)}
            </span>
            {t > 0 && <span>Tax +{t}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Renders a list of [grpId, count] with "Name × N" formatting. */
function CardCountList({
  entries,
  info,
}: {
  entries: Array<[number, number]>;
  info: Map<number, CardInfo>;
}) {
  if (entries.length === 0) return null;
  return (
    <ul className="space-y-1 mt-3 max-h-96 overflow-auto">
      {sortByCmcLandsBottom(entries, info).map(([id, count]) => (
        <li
          key={id}
          className="flex justify-between text-zinc-700 dark:text-zinc-300"
        >
          <span>{cardLabel(id, info)}</span>
          {count > 1 && (
            <span className="text-zinc-500 ml-2">× {count}</span>
          )}
        </li>
      ))}
    </ul>
  );
}

/** Decklist with live library counts + per-card "is on top of library"
 * probability. When MTGA has revealed the top to us (scry/surveil/tutor),
 * the matching card shows 100% and every other card shows 0%. Otherwise
 * the chance is the uniform `count / library_size`. */
function LibraryWithOdds({
  entries,
  info,
  librarySize,
  knownTop,
}: {
  entries: Array<[number, number]>;
  info: Map<number, CardInfo>;
  librarySize: number;
  knownTop: number | null;
}) {
  if (entries.length === 0) return null;
  // Sort: still-in-library first (by name), then exhausted entries last
  const sorted = sortByCmcLandsBottom(entries, info).filter(([, c]) => c > 0);
  return (
    <ul className="space-y-1 mt-1 text-xs">
      {sorted.map(([id, count]) => {
        let pct: number;
        if (knownTop !== null) {
          pct = id === knownTop ? 100 : 0;
        } else {
          pct = librarySize > 0 ? (count / librarySize) * 100 : 0;
        }
        const isLand = info.get(id)?.is_land === true;
        const isTop = knownTop === id;
        return (
          <li
            key={id}
            className={`flex justify-between gap-2 ${
              isTop
                ? "text-amber-700 dark:text-amber-400 font-medium"
                : "text-zinc-700 dark:text-zinc-300"
            }`}
          >
            <span className={isLand && !isTop ? "text-emerald-700 dark:text-emerald-400" : ""}>
              {isTop && "↑ "}
              {cardLabel(id, info)}
            </span>
            <span className="text-zinc-500 tabular-nums whitespace-nowrap">
              {count > 1 ? `${count}×  ` : ""}
              {pct.toFixed(2)}%
            </span>
          </li>
        );
      })}
    </ul>
  );
}

export function InGameView({ state }: Props) {
  const opponentThisGame =
    state.opponentInstances.get(state.gameNumber) ?? new Map<number, number>();
  const playerThisGame =
    state.playerInstances.get(state.gameNumber) ?? new Map<number, number>();
  const playerTax =
    state.playerSeatId !== null
      ? state.commanderTax.get(state.playerSeatId)
      : undefined;
  const opponentTax =
    state.opponentSeatId !== null
      ? state.commanderTax.get(state.opponentSeatId)
      : undefined;

  // Collect every grpId we want info for
  const allIds = new Set<number>();
  for (const grpId of opponentThisGame.values()) allIds.add(grpId);
  for (const grpId of playerThisGame.values()) allIds.add(grpId);
  if (playerTax) for (const id of playerTax.keys()) allIds.add(id);
  if (opponentTax) for (const id of opponentTax.keys()) allIds.add(id);
  if (state.playerDeck) for (const id of state.playerDeck.keys()) allIds.add(id);
  const info = useCardInfo(allIds);

  // Aggregate played cards by grpId, filtering out tokens
  function aggregate(
    instances: Map<number, number>,
  ): { entries: Array<[number, number]>; tokens: number; total: number } {
    const byGrp = new Map<number, number>();
    let tokens = 0;
    for (const grpId of instances.values()) {
      if (info.get(grpId)?.is_token) {
        tokens += 1;
      } else {
        byGrp.set(grpId, (byGrp.get(grpId) ?? 0) + 1);
      }
    }
    const entries = Array.from(byGrp.entries());
    const total = entries.reduce((sum, [, c]) => sum + c, 0);
    return { entries, tokens, total };
  }
  const opponent = aggregate(opponentThisGame);
  const player = aggregate(playerThisGame);

  // Live library — exclude commanders (in the command zone, not library)
  const libraryEntries = state.playerLibrary
    ? Array.from(state.playerLibrary.entries()).filter(
        ([id]) => !(playerTax && playerTax.has(id)),
      )
    : [];
  const librarySize = state.playerLibrarySize;

  // Lands remaining in library (uses card-info lookup; falls back to 0 if
  // the card hasn't resolved yet — the count will firm up as info arrives)
  const landsInLibrary = libraryEntries.reduce(
    (sum, [id, q]) => sum + (info.get(id)?.is_land ? q : 0),
    0,
  );
  // When the top is known, P(land on top) collapses to 100% or 0% based on
  // whether that specific card is a land.
  const knownTopIsLand =
    state.playerKnownTop !== null &&
    info.get(state.playerKnownTop)?.is_land === true;
  const landNextDrawPct =
    state.playerKnownTop !== null
      ? knownTopIsLand
        ? 100
        : 0
      : librarySize > 0
        ? (landsInLibrary / librarySize) * 100
        : 0;
  const knownTopName =
    state.playerKnownTop !== null
      ? (info.get(state.playerKnownTop)?.name ?? `Card #${state.playerKnownTop}`)
      : null;

  return (
    <div className="flex flex-col h-full">
      <IntermissionBanner state={state} />
      <div className="grid grid-cols-3 gap-4 p-4 flex-1 min-h-0">
      {/* Column 1: your decklist with live library + draw odds */}
      <section className="border border-zinc-200 dark:border-zinc-800 rounded-lg p-4 overflow-auto bg-white dark:bg-zinc-950">
        <div className="flex items-baseline justify-between mb-3">
          <h2 className="text-base font-semibold">Your library</h2>
          <span className="text-xs text-zinc-500 tabular-nums">
            {librarySize} cards
          </span>
        </div>
        {knownTopName !== null && (
          <div className="mb-2 text-xs flex justify-between text-amber-700 dark:text-amber-400">
            <span>Next draw is known:</span>
            <span className="font-medium">↑ {knownTopName}</span>
          </div>
        )}
        <div className="flex justify-between mb-3 text-xs text-zinc-500">
          <span>Next draw: land</span>
          <span>
            {state.playerKnownTop !== null
              ? `${landNextDrawPct.toFixed(0)}%`
              : `${landsInLibrary} / ${librarySize} (${landNextDrawPct.toFixed(2)}%)`}
          </span>
        </div>
        <LibraryWithOdds
          entries={libraryEntries}
          info={info}
          librarySize={librarySize}
          knownTop={state.playerKnownTop}
        />
      </section>

      {/* Column 2: your this-game state — format, game #, commander, cards played */}
      <section className="border border-zinc-200 dark:border-zinc-800 rounded-lg p-4 overflow-auto bg-white dark:bg-zinc-950">
        <h2 className="text-base font-semibold mb-3">You</h2>
        <dl className="space-y-2 text-sm">
          <div className="flex justify-between">
            <dt className="text-zinc-500">Format</dt>
            <dd>{state.format ?? "—"}</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-zinc-500">Game</dt>
            <dd>{state.gameNumber}</dd>
          </div>
        </dl>

        <CommanderList tax={playerTax} info={info} />

        <div className="mt-4 text-sm">
          <div className="mb-2 flex justify-between">
            <span className="text-zinc-500">Played this game</span>
            <span>{player.total}</span>
          </div>
          {player.tokens > 0 && (
            <div className="mb-2 flex justify-between text-xs text-zinc-500">
              <span>Tokens (not counted)</span>
              <span>{player.tokens}</span>
            </div>
          )}
          <CardCountList entries={player.entries} info={info} />
        </div>
      </section>

      {/* Column 3: opponent */}
      <section className="border border-zinc-200 dark:border-zinc-800 rounded-lg p-4 overflow-auto bg-white dark:bg-zinc-950">
        <h2 className="text-base font-semibold mb-3">
          {state.opponent?.name ?? "Opponent"}
        </h2>

        <CommanderList tax={opponentTax} info={info} />

        <div className="mt-4 text-sm">
          <div className="mb-2 flex justify-between">
            <span className="text-zinc-500">Cards seen this game</span>
            <span>{opponent.total}</span>
          </div>
          {opponent.tokens > 0 && (
            <div className="mb-2 flex justify-between text-xs text-zinc-500">
              <span>Tokens (not counted)</span>
              <span>{opponent.tokens}</span>
            </div>
          )}
          <CardCountList entries={opponent.entries} info={info} />
        </div>
      </section>
      </div>
    </div>
  );
}
