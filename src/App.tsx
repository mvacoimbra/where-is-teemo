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
  const [error, setError] = useState<string | null>(null);

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
    setError(null);
    try {
      const updated = await invoke<StatusInfo>("launch_game", { game });
      setStatus(updated);
    } catch (e) {
      setError(String(e));
    } finally {
      setLaunching(false);
    }
  }

  async function handleStop() {
    const updated = await invoke<StatusInfo>("stop_proxy");
    setStatus(updated);
    setError(null);
  }

  async function handleInstallCa() {
    setInstalling(true);
    try {
      await invoke("install_ca");
      const updated = await invoke<CertStatus>("get_cert_status");
      setCertStatus(updated);
    } catch (e) {
      setError(String(e));
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
        setError(String(e));
      }
    }
  }

  const isOffline = status.stealth_mode === "Offline";
  const isRunning = status.proxy_status === "Running";
  const needsCaInstall =
    certStatus && certStatus.ca_generated && !certStatus.ca_trusted;

  return (
    <main className="container">
      <header className="header">
        <img src="/icon.png" alt="WIT" className="app-icon" />
        <div className="header-text">
          <h1>Where Is Teemo?</h1>
          <span className="version">v0.1.0</span>
        </div>
      </header>

      {needsCaInstall && (
        <div className="banner banner-warn">
          <p>Certificado ainda nao confiavel. Instale para ativar o proxy.</p>
          <button
            className="btn btn-outline-warn"
            onClick={handleInstallCa}
            disabled={installing}
          >
            {installing ? "Instalando..." : "Confiar no Certificado"}
          </button>
        </div>
      )}

      {error && (
        <div className="banner banner-error">
          <p>{error}</p>
          <button className="dismiss" onClick={() => setError(null)}>
            x
          </button>
        </div>
      )}

      <div className="card">
        <button
          className={`stealth-toggle ${isOffline ? "invisible" : "online"}`}
          onClick={toggleStealth}
        >
          <span className="toggle-dot" />
          <div className="toggle-text">
            <span className="toggle-label">
              {isOffline ? "Invisivel" : "Online"}
            </span>
            <span className="toggle-hint">
              {isOffline
                ? "Seus amigos nao te veem online"
                : "Voce esta visivel para amigos"}
            </span>
          </div>
        </button>
      </div>

      <div className="card">
        <label className="field-label">Regiao</label>
        <select
          className="select"
          value={selectedRegion}
          onChange={(e) => handleRegionChange(e.target.value)}
        >
          <option value="">Auto-detectar</option>
          {regions.map((r) => (
            <option key={r.code} value={r.code}>
              {r.name}
            </option>
          ))}
        </select>
      </div>

      {!isRunning ? (
        <div className="launch-section">
          <div className="game-buttons">
            <button
              className="btn btn-game btn-lol"
              onClick={() => handleLaunch("league_of_legends")}
              disabled={launching}
            >
              {launching ? "Abrindo..." : "League of Legends"}
            </button>
            <button
              className="btn btn-game btn-val"
              onClick={() => handleLaunch("valorant")}
              disabled={launching}
            >
              {launching ? "Abrindo..." : "VALORANT"}
            </button>
          </div>
          <p className="launch-hint">
            O jogo precisa ser aberto pelo WIT para o modo invisivel funcionar.
            Se ja estiver aberto, ele sera reiniciado.
          </p>
        </div>
      ) : (
        <div className="running-section">
          <div className="running-info">
            <span className="running-dot" />
            <span className="running-label">
              Proxy ativo
              {status.connected_game &&
                ` — ${status.connected_game.replace("_", " ")}`}
            </span>
          </div>
          <button className="btn btn-stop" onClick={handleStop}>
            Parar
          </button>
        </div>
      )}

      <footer className="footer">
        <span>
          {certStatus?.ca_trusted ? "CA OK" : "CA pendente"}
          {" · "}
          {isRunning ? "Proxy ativo" : "Proxy parado"}
        </span>
      </footer>
    </main>
  );
}

export default App;
