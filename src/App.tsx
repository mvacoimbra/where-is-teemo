import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { StatusInfo } from "./types";

function App() {
  const [status, setStatus] = useState<StatusInfo>({
    stealth_mode: "Offline",
    proxy_status: "Idle",
    connected_game: null,
  });

  useEffect(() => {
    invoke<StatusInfo>("get_status").then(setStatus);
  }, []);

  async function toggleStealth() {
    const newMode = status.stealth_mode === "Offline" ? "online" : "offline";
    const updated = await invoke<StatusInfo>("set_stealth_mode", {
      mode: newMode,
    });
    setStatus(updated);
  }

  async function handleLaunch(game: string) {
    const result = await invoke<string>("launch_game", { game });
    console.log(result);
  }

  const isOffline = status.stealth_mode === "Offline";

  return (
    <main className="container">
      <h1>Where Is Teemo?</h1>
      <p className="subtitle">Nowhere to be found.</p>

      <div className="status-section">
        <button
          className={`stealth-toggle ${isOffline ? "active" : ""}`}
          onClick={toggleStealth}
        >
          <span className={`status-dot ${isOffline ? "offline" : "online"}`} />
          {isOffline ? "Invisible" : "Online"}
        </button>
        <p className="status-hint">
          {isOffline
            ? "You will appear offline to friends"
            : "You are visible to friends"}
        </p>
      </div>

      <div className="game-buttons">
        <button
          className="game-btn lol"
          onClick={() => handleLaunch("league_of_legends")}
        >
          Launch LoL
        </button>
        <button
          className="game-btn val"
          onClick={() => handleLaunch("valorant")}
        >
          Launch VALORANT
        </button>
      </div>

      <div className="proxy-status">
        {status.proxy_status === "Running"
          ? "Proxy active"
          : status.proxy_status === "Idle"
            ? "Proxy idle"
            : `Error: ${typeof status.proxy_status === "object" ? status.proxy_status.Error : ""}`}
      </div>
    </main>
  );
}

export default App;
