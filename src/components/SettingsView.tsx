import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type ExportState = { status: "idle" } | { status: "busy" } | { status: "done"; path: string } | { status: "error"; msg: string };

interface SettingsSnapshot {
  player_id: string | null;
  player_name: string | null;
  track_deck_history: boolean;
  backup_on_launch: boolean;
}

export function SettingsView() {
  const [snap, setSnap] = useState<SettingsSnapshot | null>(null);
  const [confirmingReset, setConfirmingReset] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [exportState, setExportState] = useState<ExportState>({ status: "idle" });
  const [cardImageSource, setCardImageSourceState] = useState<"scryfall" | "gatherer">(
    () => (localStorage.getItem("cardImageSource") as "scryfall" | "gatherer" | null) ?? "scryfall",
  );

  function setCardImageSource(src: "scryfall" | "gatherer") {
    localStorage.setItem("cardImageSource", src);
    setCardImageSourceState(src);
  }

  function handleExport() {
    setExportState({ status: "busy" });
    invoke<string | null>("export_match_history")
      .then((path) => {
        if (path === null) {
          setExportState({ status: "idle" }); // user cancelled dialog
        } else {
          setExportState({ status: "done", path });
        }
      })
      .catch((e) => setExportState({ status: "error", msg: String(e) }));
  }

  function refresh() {
    invoke<SettingsSnapshot>("get_settings")
      .then(setSnap)
      .catch(() => {});
  }

  useEffect(() => {
    refresh();
  }, []);

  async function toggleSetting(key: string, value: boolean) {
    try {
      await invoke("set_app_setting", {
        key,
        value: value ? "true" : "false",
      });
      refresh();
    } catch {
      // ignore
    }
  }

  async function doReset() {
    setResetting(true);
    try {
      await invoke("reset_stats");
    } catch {
      // ignore
    }
    setResetting(false);
    setConfirmingReset(false);
    refresh();
  }

  if (!snap) {
    return (
      <div className="p-6 max-w-2xl mx-auto">
        <p className="text-sm text-zinc-500">Loading…</p>
      </div>
    );
  }

  const noUser = snap.player_id === null;

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-8">
      <section>
        <h2 className="text-lg font-semibold mb-3">Active player</h2>
        <div className="rounded-lg border border-zinc-200 dark:border-zinc-800 p-4">
          {noUser ? (
            <div>
              <div className="text-sm font-medium">No player detected yet</div>
              <p className="text-xs text-zinc-500 mt-1">
                Stats start recording as soon as MTGA logs in. The app
                identifies the active player automatically from the MTGA log
                and keeps a separate stats database per user — accounts on
                this PC don't share data.
              </p>
            </div>
          ) : (
            <div>
              <div className="text-sm text-zinc-500">Currently recording stats for</div>
              <div className="text-base font-medium mt-1">
                {snap.player_name ?? snap.player_id}
              </div>
              <p className="text-xs text-zinc-500 mt-2">
                Detected from MTGA log activity. Stats live at a per-user
                database; if a different MTGA account logs in on this PC, the
                app swaps to that user's database automatically.
              </p>
            </div>
          )}
        </div>
      </section>

      <section>
        <h2 className="text-lg font-semibold mb-3">App settings</h2>
        <div className="rounded-lg border border-zinc-200 dark:border-zinc-800 p-4 space-y-3">
          <ToggleRow
            label="Save deck history"
            description="Capture decks from the MTGA log so the Decks tab stays populated."
            value={snap.track_deck_history}
            onChange={(v) => toggleSetting("track_deck_history", v)}
            disabled={noUser}
          />
          <ToggleRow
            label="Backup database on user switch"
            description="Copies stats.db to stats.db.bak whenever the active user changes (including app launch)."
            value={snap.backup_on_launch}
            onChange={(v) => toggleSetting("backup_on_launch", v)}
            disabled={noUser}
          />
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="text-sm font-medium">Card image source</div>
              <p className="text-xs text-zinc-500 mt-0.5">
                Where to load card images when hovering a card name.
                Gatherer falls back to Scryfall for Arena-only cards.
              </p>
            </div>
            <div className="flex rounded border border-zinc-200 dark:border-zinc-700 overflow-hidden text-xs shrink-0">
              {(["scryfall", "gatherer"] as const).map((src) => (
                <button
                  key={src}
                  type="button"
                  onClick={() => setCardImageSource(src)}
                  className={`px-3 py-1 capitalize ${
                    cardImageSource === src
                      ? "bg-zinc-200 dark:bg-zinc-700 font-medium"
                      : "hover:bg-zinc-100 dark:hover:bg-zinc-800 text-zinc-500"
                  }`}
                >
                  {src.charAt(0).toUpperCase() + src.slice(1)}
                </button>
              ))}
            </div>
          </div>
          {noUser && (
            <p className="text-xs text-zinc-500">
              Settings can be changed once a player is active.
            </p>
          )}
        </div>
      </section>

      <section>
        <h2 className="text-lg font-semibold mb-3">Data</h2>
        <div className="rounded-lg border border-zinc-200 dark:border-zinc-800 p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="text-sm font-medium">Export match history</div>
              <p className="text-xs text-zinc-500 mt-1">
                Saves all matches as a JSON file to your Desktop (or app data
                folder if Desktop isn't found).
              </p>
              {exportState.status === "done" && (
                <p className="text-xs text-green-600 dark:text-green-400 mt-1" title={exportState.path}>
                  Saved: {exportState.path.split(/[\\/]/).pop()}
                </p>
              )}
              {exportState.status === "error" && (
                <p className="text-xs text-red-500 mt-1">{exportState.msg}</p>
              )}
            </div>
            <button
              type="button"
              onClick={handleExport}
              disabled={exportState.status === "busy" || noUser}
              className="text-xs px-3 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
            >
              {exportState.status === "busy" ? "Exporting…" : "Export JSON"}
            </button>
          </div>
        </div>
      </section>

      <section>
        <h2 className="text-lg font-semibold mb-3">Danger zone</h2>
        <div className="rounded-lg border border-red-200 dark:border-red-900 p-4">
          <div className="flex items-baseline justify-between">
            <div>
              <div className="text-sm font-medium">Reset stats</div>
              <p className="text-xs text-zinc-500 mt-1">
                Wipes match history and per-game data{" "}
                <strong>for the active player only</strong>. Settings and
                deck snapshots stay. Other users on this PC are unaffected.
              </p>
            </div>
            {confirmingReset ? (
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={doReset}
                  disabled={resetting}
                  className="text-xs px-2 py-1 rounded bg-red-600 text-white hover:bg-red-700"
                >
                  {resetting ? "Resetting…" : "Yes, reset"}
                </button>
                <button
                  type="button"
                  onClick={() => setConfirmingReset(false)}
                  disabled={resetting}
                  className="text-xs px-2 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setConfirmingReset(true)}
                disabled={noUser}
                className="text-xs px-2 py-1 rounded border border-red-300 dark:border-red-800 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Reset stats…
              </button>
            )}
          </div>
        </div>
      </section>
    </div>
  );
}

function ToggleRow({
  label,
  description,
  value,
  onChange,
  disabled,
}: {
  label: string;
  description: string;
  value: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div>
        <div className="text-sm font-medium">{label}</div>
        <p className="text-xs text-zinc-500 mt-0.5">{description}</p>
      </div>
      <button
        type="button"
        onClick={() => !disabled && onChange(!value)}
        disabled={disabled}
        aria-pressed={value}
        className={`relative shrink-0 inline-flex h-6 w-11 rounded-full transition-colors disabled:opacity-50 ${
          value ? "bg-blue-600" : "bg-zinc-300 dark:bg-zinc-700"
        }`}
      >
        <span
          className={`inline-block h-5 w-5 mt-0.5 transform rounded-full bg-white transition-transform ${
            value ? "translate-x-5" : "translate-x-0.5"
          }`}
        />
      </button>
    </div>
  );
}
