export type StealthMode = "Online" | "Offline";

export type ProxyStatus =
  | "Idle"
  | "Running"
  | { Error: string };

export type StatusInfo = {
  stealth_mode: StealthMode;
  proxy_status: ProxyStatus;
  connected_game: string | null;
};
