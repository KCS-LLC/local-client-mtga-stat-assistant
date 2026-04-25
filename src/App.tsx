import { useEffect, useState } from "react";
import { StatusBar } from "./components/StatusBar";
import { OutOfGameView } from "./components/OutOfGameView";
import { InGameView } from "./components/InGameView";
import { useMatchState } from "./hooks/useMatchState";

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

function App() {
  const state = useMatchState();
  const { theme, toggle } = useTheme();

  return (
    <div className="h-full flex flex-col bg-zinc-50 dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100">
      <StatusBar
        mtgaRunning={state.mtgaRunning}
        inMatch={state.inMatch}
        theme={theme}
        onToggleTheme={toggle}
      />
      <main className="flex-1 overflow-auto">
        {state.inMatch ? <InGameView state={state} /> : <OutOfGameView />}
      </main>
    </div>
  );
}

export default App;
