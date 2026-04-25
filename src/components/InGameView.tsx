import type { MatchState } from "../hooks/useMatchState";
import { useCardInfo, type CardInfo } from "../hooks/useCardNames";

interface Props {
  state: MatchState;
}

function cardLabel(id: number, info: Map<number, CardInfo>): string {
  return info.get(id)?.name ?? `Card #${id}`;
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

export function InGameView({ state }: Props) {
  const cardsThisGame =
    state.opponentCards.get(state.gameNumber) ?? new Set<number>();
  const playerTax =
    state.playerSeatId !== null
      ? state.commanderTax.get(state.playerSeatId)
      : undefined;
  const opponentTax =
    state.opponentSeatId !== null
      ? state.commanderTax.get(state.opponentSeatId)
      : undefined;

  // Collect every grpId we want info for, then resolve them all at once
  const allIds = new Set<number>(cardsThisGame);
  if (playerTax) for (const id of playerTax.keys()) allIds.add(id);
  if (opponentTax) for (const id of opponentTax.keys()) allIds.add(id);
  const info = useCardInfo(allIds);

  // Filter tokens out of the opponent's "cards seen" list — they don't exist
  // outside the game and would clutter the deck-tracking view. Tokens we
  // haven't resolved yet (info missing) get optimistically shown; once the
  // lookup resolves and is_token = true, they drop out.
  const realCardsThisGame = new Set<number>();
  let tokenCount = 0;
  for (const id of cardsThisGame) {
    if (info.get(id)?.is_token) {
      tokenCount += 1;
    } else {
      realCardsThisGame.add(id);
    }
  }

  return (
    <div className="grid grid-cols-2 gap-4 p-4 h-full">
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

        <div className="mt-6 text-sm italic text-zinc-500">
          Draw odds — coming soon
        </div>
      </section>

      <section className="border border-zinc-200 dark:border-zinc-800 rounded-lg p-4 overflow-auto bg-white dark:bg-zinc-950">
        <h2 className="text-base font-semibold mb-3">
          {state.opponent?.name ?? "Opponent"}
        </h2>

        <CommanderList tax={opponentTax} info={info} />

        <div className="mt-4 text-sm">
          <div className="mb-2 flex justify-between">
            <span className="text-zinc-500">Cards seen this game</span>
            <span>{realCardsThisGame.size}</span>
          </div>
          {tokenCount > 0 && (
            <div className="mb-2 flex justify-between text-xs text-zinc-500">
              <span>Tokens (not counted)</span>
              <span>{tokenCount}</span>
            </div>
          )}
          {realCardsThisGame.size > 0 && (
            <ul className="space-y-1 mt-3 max-h-96 overflow-auto">
              {Array.from(realCardsThisGame).map((id) => (
                <li key={id} className="text-zinc-700 dark:text-zinc-300">
                  {cardLabel(id, info)}
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>
    </div>
  );
}
