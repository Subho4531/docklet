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
  llmModels: LlmModelConfig[];
  llmPort: number;
  llmApiKey: string;
}

export interface LlmModelConfig {
  id: string;
  displayName: string;
  repo: string;
  fileName: string;
  params: string;
  quant: string;
  contextWindow: number;
  sizeBytes: number;
}

export type LlmFit = "comfortable" | "tight" | "partial";

export interface LlmCatalogEntry {
  id: string;
  displayName: string;
  description: string;
  category: string;
  fit: LlmFit;
  params: string;
  quant: string;
  sizeBytes: number;
  contextWindow: number;
  repo: string;
  fileName: string;
}

export type LlmModelState =
  | "stopped"
  | "running"
  | "downloading"
  | "missing"
  | "error";

export interface LlmEngineStatus {
  installed: boolean;
  installing: boolean;
  version: string;
  path: string | null;
  error: string | null;
  downloadedBytes: number;
  totalBytes: number;
}

export interface LlmModelStatus {
  id: string;
  displayName: string;
  params: string;
  quant: string;
  sizeBytes: number;
  contextWindow: number;
  state: LlmModelState;
  downloadedBytes: number;
  totalBytes: number;
  endpoint: string | null;
  apiKey: string | null;
  modelName: string | null;
  error: string | null;
}

export interface LlmSnapshot {
  engine: LlmEngineStatus;
  models: LlmModelStatus[];
  port: number;
  activeModelId: string | null;
  apiKey: string;
}

export interface LlmProgressPayload {
  id: string;
  downloadedBytes: number;
  totalBytes: number;
}

export interface InstallModelRequest {
  catalogId?: string | null;
  repo?: string | null;
  fileName?: string | null;
  displayName?: string | null;
}

export type LlmOperation = "start" | "stop" | "install" | "remove";

export type ServiceOperation = "start" | "stop" | "reset";

export type MainPage = "services" | "models" | "library" | "settings";

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
