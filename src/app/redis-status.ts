export type RedisState =
  | "running"
  | "stopped"
  | "missing"
  | "starting"
  | "dockerUnavailable"
  | "unmanaged"
  | "error";

export interface RedisStatus {
  state: RedisState;
  containerState: string | null;
  port: number | null;
  url: string | null;
  dockerAvailable: boolean;
  managed: boolean;
  error: string | null;
}

export type RedisOperation = "start" | "stop" | "reset";

export type StatusTone = "ok" | "idle" | "busy" | "warn" | "danger";

export interface StatusView {
  label: string;
  detail: string;
  tone: StatusTone;
}

export interface ToastMessage {
  message: string;
  tone: "default" | "danger";
}
