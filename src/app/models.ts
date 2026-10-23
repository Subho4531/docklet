export type ServiceState =
  | "running"
  | "stopped"
  | "missing"
  | "starting"
  | "dockerUnavailable"
  | "unmanaged"
  | "error";

export type ServiceKind = "builtin" | "custom";

export interface ServiceStatus {
  id: string;
  name: string;
  icon: string;
  kind: ServiceKind;
  image: string;
  state: ServiceState;
  containerState: string | null;
  port: number | null;
  hostPort: number;
  containerPort: number;
  url: string | null;
  managed: boolean;
  error: string | null;
}

export interface ServicesSnapshot {
  dockerAvailable: boolean;
  services: ServiceStatus[];
}

export interface EnvVar {
  key: string;
  value: string;
}

export interface CustomService {
  id: string;
  name: string;
  image: string;
  containerPort: number;
  hostPort: number;
  urlTemplate: string;
  volumePath: string | null;
  env: EnvVar[];
  healthCommand: string | null;
}

export interface AppSettings {
  version: number;
  hidePopupOnBlur: boolean;
  servicePorts: Record<string, number>;
  customServices: CustomService[];
}

export type ServiceOperation = "start" | "stop" | "reset";

export type MainPage = "services" | "library" | "settings";

export interface DockerSearchResult {
  name: string;
  description: string;
  starCount: number;
  isOfficial: boolean;
}

export type ToastTone = "default" | "danger";

export interface ToastMessage {
  message: string;
  tone: ToastTone;
}

export interface ServiceErrorPayload {
  serviceId: string;
  message: string;
}
