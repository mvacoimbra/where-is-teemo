import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { CertStatus, StatusInfo } from "./types";

function App() {
  const [status, setStatus] = useState<StatusInfo>({
    stealth_mode: "Offline",
    proxy_status: "Idle",
    connected_game: null,
  });
  const [certStatus, setCertStatus] = useState<CertStatus | null>(null);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    invoke<StatusInfo>("get_status").then(setStatus);
    invoke<CertStatus>("get_cert_status").then(setCertStatus);
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

  const isOffline = status.stealth_mode === "Offline";
  const needsCaInstall = certStatus && certStatus.ca_generated && !certStatus.ca_trusted;

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
        {certStatus && (
          <span className="cert-info">
            CA: {certStatus.ca_generated ? "generated" : "missing"}
            {certStatus.ca_generated &&
              (certStatus.ca_trusted ? " (trusted)" : " (not trusted)")}
            {" | "}
          </span>
        )}
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
