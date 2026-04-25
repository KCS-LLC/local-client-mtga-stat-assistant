import type { MatchState } from "../hooks/useMatchState";

interface Props {
  state: MatchState;
}

function CommanderList({
  tax,
}: {
  tax: Map<number, number> | undefined;
}) {
  if (!tax || tax.size === 0) return null;
  return (
    <div className="mt-4">
      <h3 className="text-sm font-medium mb-2">Commander</h3>
      <ul className="text-sm space-y-1">
        {Array.from(tax.entries()).map(([id, t]) => (
          <li key={id} className="flex justify-between">
            <span className="text-zinc-600 dark:text-zinc-400">Card #{id}</span>
            <span>Tax +{t}</span>
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

        <CommanderList tax={playerTax} />

        <div className="mt-6 text-sm italic text-zinc-500">
          Draw odds — coming soon
        </div>
      </section>

      <section className="border border-zinc-200 dark:border-zinc-800 rounded-lg p-4 overflow-auto bg-white dark:bg-zinc-950">
        <h2 className="text-base font-semibold mb-3">
          {state.opponent?.name ?? "Opponent"}
        </h2>

        <CommanderList tax={opponentTax} />

        <div className="mt-4 text-sm">
          <div className="mb-2 flex justify-between">
            <span className="text-zinc-500">Cards seen this game</span>
            <span>{cardsThisGame.size}</span>
          </div>
          {cardsThisGame.size > 0 && (
            <ul className="space-y-1 mt-3 max-h-96 overflow-auto">
              {Array.from(cardsThisGame).map((id) => (
                <li key={id} className="text-zinc-600 dark:text-zinc-400">
                  Card #{id}
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>
    </div>
  );
}
