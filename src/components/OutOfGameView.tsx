import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeckWL, MatchRecord } from "../types/db";

interface Props {
  onOpenDeck?: (deckId: string) => void;
}

export function OutOfGameView({ onOpenDeck }: Props) {
  const [stats, setStats] = useState<DeckWL[]>([]);
  const [history, setHistory] = useState<MatchRecord[]>([]);

  useEffect(() => {
    invoke<DeckWL[]>("get_wl_stats")
      .then(setStats)
      .catch(() => {});
    invoke<MatchRecord[]>("get_match_history")
      .then(setHistory)
      .catch(() => {});
  }, []);

  // Compute going-first/second W/L from match history
  const firstRecord = { wins: 0, losses: 0 };
  const secondRecord = { wins: 0, losses: 0 };
  for (const m of history) {
    if (m.played_first === null || (m.result !== "Win" && m.result !== "Loss")) continue;
    const bucket = m.played_first ? firstRecord : secondRecord;
    if (m.result === "Win") bucket.wins++;
    else bucket.losses++;
  }
  const hasPlayOrder = firstRecord.wins + firstRecord.losses + secondRecord.wins + secondRecord.losses > 0;

  return (
    <div className="p-6 max-w-5xl mx-auto space-y-8">
      <section>
        <h2 className="text-lg font-semibold mb-3">Win/Loss by deck</h2>
        {stats.length === 0 ? (
          <p className="text-sm text-zinc-500">No completed matches yet.</p>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-zinc-200 dark:border-zinc-800">
                <th className="text-left py-2">Deck</th>
                <th className="text-right py-2">Wins</th>
                <th className="text-right py-2">Losses</th>
                <th className="text-right py-2">Win rate</th>
              </tr>
            </thead>
            <tbody>
              {stats.map((s) => {
                const total = s.wins + s.losses;
                const rate = total > 0 ? Math.round((s.wins / total) * 100) : 0;
                return (
                  <tr
                    key={s.deck_name}
                    className="border-b border-zinc-100 dark:border-zinc-900"
                  >
                    <td className="py-2">{s.deck_name}</td>
                    <td className="py-2 text-right">{s.wins}</td>
                    <td className="py-2 text-right">{s.losses}</td>
                    <td className="py-2 text-right">{rate}%</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </section>

      {hasPlayOrder && (
        <section>
          <h2 className="text-lg font-semibold mb-3">Win/Loss by play order</h2>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-zinc-200 dark:border-zinc-800">
                <th className="text-left py-2">Order</th>
                <th className="text-right py-2">Wins</th>
                <th className="text-right py-2">Losses</th>
                <th className="text-right py-2">Win rate</th>
              </tr>
            </thead>
            <tbody>
              {([
                { label: "Went first", rec: firstRecord },
                { label: "Went second", rec: secondRecord },
              ] as const).map(({ label, rec }) => {
                const total = rec.wins + rec.losses;
                const rate = total > 0 ? Math.round((rec.wins / total) * 100) : 0;
                return (
                  <tr key={label} className="border-b border-zinc-100 dark:border-zinc-900">
                    <td className="py-2">{label}</td>
                    <td className="py-2 text-right">{rec.wins}</td>
                    <td className="py-2 text-right">{rec.losses}</td>
                    <td className="py-2 text-right">{total > 0 ? `${rate}%` : "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </section>
      )}

      <section>
        <h2 className="text-lg font-semibold mb-3">Recent matches</h2>
        {history.length === 0 ? (
          <p className="text-sm text-zinc-500">No matches yet.</p>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-zinc-200 dark:border-zinc-800">
                <th className="text-left py-2">Date</th>
                <th className="text-left py-2">Format</th>
                <th className="text-left py-2">Opponent</th>
                <th className="text-left py-2">Deck</th>
                <th className="text-right py-2">Order</th>
                <th className="text-right py-2">Result</th>
              </tr>
            </thead>
            <tbody>
              {history.map((m) => (
                <tr
                  key={m.match_id}
                  className="border-b border-zinc-100 dark:border-zinc-900"
                >
                  <td className="py-2">
                    {new Date(m.started_at).toLocaleDateString()}
                  </td>
                  <td className="py-2">{m.format}</td>
                  <td className="py-2">{m.opponent_name}</td>
                  <td className="py-2">
                    {m.deck_name && m.deck_id && onOpenDeck ? (
                      <button
                        type="button"
                        onClick={() => onOpenDeck(m.deck_id!)}
                        className="text-blue-600 dark:text-blue-400 hover:underline"
                      >
                        {m.deck_name}
                      </button>
                    ) : (
                      (m.deck_name ?? "—")
                    )}
                  </td>
                  <td className="py-2 text-right text-zinc-500">
                    {m.played_first === true ? "1st" : m.played_first === false ? "2nd" : "—"}
                  </td>
                  <td
                    className={`py-2 text-right font-medium ${
                      m.result === "Win"
                        ? "text-green-600 dark:text-green-400"
                        : m.result === "Loss"
                          ? "text-red-600 dark:text-red-400"
                          : ""
                    }`}
                  >
                    {m.result ?? "In progress"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
