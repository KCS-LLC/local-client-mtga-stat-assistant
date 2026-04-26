import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  mtgaRunning: boolean;
  inMatch: boolean;
  eventCount: number;
  lastEventType: string | null;
  theme: "light" | "dark";
  onToggleTheme: () => void;
}

export function StatusBar({
  mtgaRunning,
  inMatch,
  eventCount,
  lastEventType,
  theme,
  onToggleTheme,
}: Props) {
  const [copyState, setCopyState] = useState<"idle" | "copying" | "ok" | "err">(
    "idle",
  );
  const [copyMsg, setCopyMsg] = useState<string>("");

  async function copyLogs() {
    setCopyState("copying");
    setCopyMsg("");
    try {
      const result = await invoke<string>("copy_logs_for_review");
      setCopyState("ok");
      setCopyMsg(result);
    } catch (e) {
      setCopyState("err");
      setCopyMsg(String(e));
    }
    setTimeout(() => {
      setCopyState("idle");
      setCopyMsg("");
    }, 4000);
  }
  const status: "red" | "yellow" | "green" = !mtgaRunning
    ? "red"
    : inMatch
      ? "green"
      : "yellow";

  const label = !mtgaRunning
    ? "MTGA not running"
    : inMatch
      ? "In match"
      : "MTGA idle";

  const dotColor = {
    red: "bg-red-500",
    yellow: "bg-amber-500",
    green: "bg-green-500",
  }[status];

  return (
    <header className="flex items-center justify-between px-4 py-2 border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
      <div className="flex items-center gap-3">
        <div className={`w-3 h-3 rounded-full ${dotColor}`} />
        <span className="text-sm font-medium">{label}</span>
        {!mtgaRunning && (
          <button
            type="button"
            onClick={() => invoke("launch_mtga", { path: null }).catch(() => {})}
            className="ml-2 text-xs px-2 py-1 rounded bg-blue-600 text-white hover:bg-blue-700"
          >
            Launch MTGA
          </button>
        )}
      </div>
      <div className="flex items-center gap-3">
        <span className="text-xs text-zinc-500 font-mono">
          events: {eventCount}
          {lastEventType ? ` · last: ${lastEventType}` : ""}
        </span>
        <button
          type="button"
          onClick={copyLogs}
          disabled={copyState === "copying"}
          title={copyMsg || "Snapshot Player.log and debug.log to working folder"}
          className={`text-xs px-2 py-1 rounded ${
            copyState === "ok"
              ? "bg-green-600 text-white"
              : copyState === "err"
                ? "bg-red-600 text-white"
                : "border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
          }`}
        >
          {copyState === "copying"
            ? "Copying…"
            : copyState === "ok"
              ? "Copied ✓"
              : copyState === "err"
                ? "Failed ✗"
                : "Copy logs"}
        </button>
        <button
          type="button"
          onClick={onToggleTheme}
          className="text-sm px-2 py-1 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
          aria-label="Toggle theme"
        >
          {theme === "dark" ? "Light" : "Dark"}
        </button>
      </div>
    </header>
  );
}
