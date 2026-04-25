import { invoke } from "@tauri-apps/api/core";

interface Props {
  mtgaRunning: boolean;
  inMatch: boolean;
  theme: "light" | "dark";
  onToggleTheme: () => void;
}

export function StatusBar({ mtgaRunning, inMatch, theme, onToggleTheme }: Props) {
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
      <button
        type="button"
        onClick={onToggleTheme}
        className="text-sm px-2 py-1 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
        aria-label="Toggle theme"
      >
        {theme === "dark" ? "Light" : "Dark"}
      </button>
    </header>
  );
}
