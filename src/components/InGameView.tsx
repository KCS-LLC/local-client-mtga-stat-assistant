import type { MatchState } from "../hooks/useMatchState";
import { useCardInfo, type CardInfo } from "../hooks/useCardNames";

interface Props {
  state: MatchState;
}

function cardLabel(id: number, info: Map<number, CardInfo>): string {
  return info.get(id)?.name ?? `Card #${id}`;
}

/** Sort entries [grpId, count] alphabetically by resolved card name. */
function sortByName(
  entries: Array<[number, number]>,
  info: Map<number, CardInfo>,
): Array<[number, number]> {
  return entries.slice().sort((a, b) => {
    const an = info.get(a[0])?.name ?? `Card #${a[0]}`;
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
      {sortByName(entries, info).map(([id, count]) => (
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

export function InGameView({ state }: Props) {
  const instancesThisGame =
    state.opponentInstances.get(state.gameNumber) ?? new Map<number, number>();
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
  for (const grpId of instancesThisGame.values()) allIds.add(grpId);
  if (playerTax) for (const id of playerTax.keys()) allIds.add(id);
  if (opponentTax) for (const id of opponentTax.keys()) allIds.add(id);
  if (state.playerDeck) for (const id of state.playerDeck.keys()) allIds.add(id);
  const info = useCardInfo(allIds);

  // Aggregate opponent's seen cards by grpId, filtering out tokens
  const opponentByGrp = new Map<number, number>();
  let tokenInstances = 0;
  for (const grpId of instancesThisGame.values()) {
    if (info.get(grpId)?.is_token) {
      tokenInstances += 1;
    } else {
      opponentByGrp.set(grpId, (opponentByGrp.get(grpId) ?? 0) + 1);
    }
  }
  const opponentEntries = Array.from(opponentByGrp.entries());
  const totalRealOpponentInstances = opponentEntries.reduce(
    (sum, [, c]) => sum + c,
    0,
  );

  // Player deck — exclude commander (shown separately)
  const playerDeckEntries = state.playerDeck
    ? Array.from(state.playerDeck.entries()).filter(
        ([id]) => !(playerTax && playerTax.has(id)),
      )
    : [];
  const playerDeckTotal = playerDeckEntries.reduce((sum, [, q]) => sum + q, 0);

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

        <div className="mt-4 text-sm">
          <div className="mb-2 flex justify-between">
            <span className="text-zinc-500">Deck</span>
            <span>{playerDeckTotal} cards</span>
          </div>
          <CardCountList entries={playerDeckEntries} info={info} />
        </div>

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
            <span>{totalRealOpponentInstances}</span>
          </div>
          {tokenInstances > 0 && (
            <div className="mb-2 flex justify-between text-xs text-zinc-500">
              <span>Tokens (not counted)</span>
              <span>{tokenInstances}</span>
            </div>
          )}
          <CardCountList entries={opponentEntries} info={info} />
        </div>
      </section>
    </div>
  );
}
