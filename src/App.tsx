import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { CertStatus, RegionInfo, StatusInfo } from "./types";

function App() {
  const [status, setStatus] = useState<StatusInfo>({
    stealth_mode: "Offline",
    proxy_status: "Idle",
    connected_game: null,
  });
  const [certStatus, setCertStatus] = useState<CertStatus | null>(null);
  const [regions, setRegions] = useState<RegionInfo[]>([]);
  const [selectedRegion, setSelectedRegion] = useState("");
  const [installing, setInstalling] = useState(false);
  const [launching, setLaunching] = useState(false);

  useEffect(() => {
    invoke<StatusInfo>("get_status").then(setStatus);
    invoke<CertStatus>("get_cert_status").then(setCertStatus);
    invoke<RegionInfo[]>("get_regions").then(setRegions);
  }, []);

  async function toggleStealth() {
    const newMode = status.stealth_mode === "Offline" ? "online" : "offline";
    const updated = await invoke<StatusInfo>("set_stealth_mode", {
      mode: newMode,
    });
    setStatus(updated);
  }

  async function handleLaunch(game: string) {
    setLaunching(true);
    try {
      const updated = await invoke<StatusInfo>("launch_game", { game });
      setStatus(updated);
    } catch (e) {
      console.error("Launch failed:", e);
    } finally {
      setLaunching(false);
    }
  }

  async function handleStop() {
    const updated = await invoke<StatusInfo>("stop_proxy");
    setStatus(updated);
  }

  async function handleInstallCa() {
    setInstalling(true);
    try {
      await invoke("install_ca");
      const updated = await invoke<CertStatus>("get_cert_status");
      setCertStatus(updated);
    } catch (e) {
      console.error("Failed to install CA:", e);
    } finally {
      setInstalling(false);
    }
  }

  async function handleRegionChange(code: string) {
    setSelectedRegion(code);
    if (code) {
      try {
        await invoke("set_region", { region: code });
      } catch (e) {
        console.error("Failed to set region:", e);
      }
    }
  }

  const isOffline = status.stealth_mode === "Offline";
  const isRunning = status.proxy_status === "Running";
  const needsCaInstall =
    certStatus && certStatus.ca_generated && !certStatus.ca_trusted;

  return (
    <main className="container">
      <h1>Where Is Teemo?</h1>
      <p className="subtitle">Nowhere to be found.</p>

      {needsCaInstall && (
        <div className="cert-banner">
          <p>Certificate not trusted yet. Install it to enable the proxy.</p>
          <button
            className="cert-btn"
            onClick={handleInstallCa}
            disabled={installing}
          >
            {installing ? "Installing..." : "Trust Certificate"}
          </button>
        </div>
      )}

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

      <div className="region-selector">
        <select
          value={selectedRegion}
          onChange={(e) => handleRegionChange(e.target.value)}
        >
          <option value="">Auto-detect region</option>
          {regions.map((r) => (
            <option key={r.code} value={r.code}>
              {r.name}
            </option>
          ))}
        </select>
      </div>

      {!isRunning ? (
        <div className="game-buttons">
          <button
            className="game-btn lol"
            onClick={() => handleLaunch("league_of_legends")}
            disabled={launching}
          >
            {launching ? "Launching..." : "Launch LoL"}
          </button>
          <button
            className="game-btn val"
            onClick={() => handleLaunch("valorant")}
            disabled={launching}
          >
            {launching ? "Launching..." : "Launch VALORANT"}
          </button>
        </div>
      ) : (
        <div className="game-buttons">
          <button className="game-btn stop" onClick={handleStop}>
            Stop Proxy
          </button>
          {status.connected_game && (
            <span className="running-game">
              Playing: {status.connected_game.replace("_", " ")}
            </span>
          )}
        </div>
      )}

      <div className="proxy-status">
        {certStatus && (
          <span className="cert-info">
            CA: {certStatus.ca_generated ? "generated" : "missing"}
            {certStatus.ca_generated &&
              (certStatus.ca_trusted ? " (trusted)" : " (not trusted)")}
            {" | "}
          </span>
        )}
        {isRunning
          ? "Proxy active"
          : status.proxy_status === "Idle"
            ? "Proxy idle"
            : `Error: ${typeof status.proxy_status === "object" ? status.proxy_status.Error : ""}`}
      </div>
    </main>
  );
}

export default App;
