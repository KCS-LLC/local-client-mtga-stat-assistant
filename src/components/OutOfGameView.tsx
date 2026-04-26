import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeckWL, MatchRecord } from "../types/db";

export function OutOfGameView() {
  const [stats, setStats] = useState<DeckWL[]>([]);
  const [history, setHistory] = useState<MatchRecord[]>([]);
  const [confirming, setConfirming] = useState(false);
  const [resetting, setResetting] = useState(false);

  function refresh() {
    invoke<DeckWL[]>("get_wl_stats")
      .then(setStats)
      .catch(() => {});
    invoke<MatchRecord[]>("get_match_history")
      .then(setHistory)
      .catch(() => {});
  }

  useEffect(() => {
    refresh();
  }, []);

  async function doReset() {
    setResetting(true);
    try {
      await invoke("reset_stats");
      refresh();
    } catch {
      // ignore — stale data will linger but it's not destructive
    }
    setResetting(false);
    setConfirming(false);
  }

  return (
    <div className="p-6 max-w-5xl mx-auto space-y-8">
      <section>
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold">Win/Loss by deck</h2>
          {confirming ? (
            <div className="flex items-center gap-2 text-xs">
              <span className="text-zinc-500">Delete all match history?</span>
              <button
                type="button"
                onClick={doReset}
                disabled={resetting}
                className="px-2 py-1 rounded bg-red-600 text-white hover:bg-red-700"
              >
                {resetting ? "Resetting…" : "Yes, reset"}
              </button>
              <button
                type="button"
                onClick={() => setConfirming(false)}
                disabled={resetting}
                className="px-2 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => setConfirming(true)}
              className="text-xs px-2 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 text-zinc-600 dark:text-zinc-400"
              title="Wipe matches, games, and opponent_cards (keeps deck snapshots and settings)"
            >
              Reset stats
            </button>
          )}
        </div>
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
                  <td className="py-2">{m.deck_name ?? "—"}</td>
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
