import { useEffect, useState } from "react";
import { StatusBar } from "./components/StatusBar";
import { OutOfGameView } from "./components/OutOfGameView";
import { InGameView } from "./components/InGameView";
import { DeckExplorer } from "./components/DeckExplorer";
import { SettingsView } from "./components/SettingsView";
import { useMatchState } from "./hooks/useMatchState";

type OutOfGameTab = "stats" | "decks" | "settings";

function useTheme() {
  const [theme, setTheme] = useState<"light" | "dark">(() => {
    const saved = localStorage.getItem("theme");
    if (saved === "light" || saved === "dark") return saved;
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  });

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem("theme", theme);
  }, [theme]);

  return {
    theme,
    toggle: () => setTheme((t) => (t === "dark" ? "light" : "dark")),
  };
}

function OutOfGameShell() {
  const [tab, setTab] = useState<OutOfGameTab>("stats");
  const [selectedDeckId, setSelectedDeckId] = useState<string | null>(null);

  function openDeck(deckId: string) {
    setSelectedDeckId(deckId);
    setTab("decks");
  }

  return (
    <div>
      <nav className="border-b border-zinc-200 dark:border-zinc-800 px-6 flex gap-1">
        <TabButton label="Stats" active={tab === "stats"} onClick={() => setTab("stats")} />
        <TabButton label="Decks" active={tab === "decks"} onClick={() => setTab("decks")} />
        <TabButton
          label="Settings"
          active={tab === "settings"}
          onClick={() => setTab("settings")}
        />
      </nav>
      {tab === "stats" && <OutOfGameView onOpenDeck={openDeck} />}
      {tab === "decks" && <DeckExplorer initialDeckId={selectedDeckId} />}
      {tab === "settings" && <SettingsView />}
    </div>
  );
}

function TabButton({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${
        active
          ? "border-blue-600 text-blue-600 dark:text-blue-400 dark:border-blue-400"
          : "border-transparent text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
      }`}
    >
      {label}
    </button>
  );
}

function App() {
  const state = useMatchState();
  const { theme, toggle } = useTheme();

  return (
    <div className="h-full flex flex-col bg-zinc-50 dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100">
      <StatusBar
        mtgaRunning={state.mtgaRunning}
        inMatch={state.inMatch}
        eventCount={state.eventCount}
        lastEventType={state.lastEventType}
        theme={theme}
        onToggleTheme={toggle}
      />
      <main className="flex-1 overflow-auto">
        {state.inMatch ? <InGameView state={state} /> : <OutOfGameShell />}
      </main>
    </div>
  );
}

export default App;
