import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppSettings,
  DockerSearchResult,
  InstallModelRequest,
  LlmCatalogEntry,
  LlmProgressPayload,
  LlmSnapshot,
  MainPage,
  ServiceErrorPayload,
  ServicesSnapshot,
  ToastMessage,
} from "./models";

@Injectable({ providedIn: "root" })
export class DockletApi {
  getServices(): Promise<ServicesSnapshot> {
    return invoke<ServicesSnapshot>("get_services_status");
  }

  startService(serviceId: string): Promise<ServicesSnapshot> {
    return invoke<ServicesSnapshot>("start_service", { serviceId });
  }

  stopService(serviceId: string): Promise<ServicesSnapshot> {
    return invoke<ServicesSnapshot>("stop_service", { serviceId });
  }

  resetService(serviceId: string): Promise<ServicesSnapshot> {
    return invoke<ServicesSnapshot>("reset_service", { serviceId });
  }

  copyServiceUrl(serviceId: string): Promise<string> {
    return invoke<string>("copy_service_url", { serviceId });
  }

  getSettings(): Promise<AppSettings> {
    return invoke<AppSettings>("get_settings");
  }

  saveSettings(settings: AppSettings): Promise<AppSettings> {
    return invoke<AppSettings>("save_settings", { settings });
  }

  removeCustomService(serviceId: string): Promise<AppSettings> {
    return invoke<AppSettings>("remove_custom_service", { serviceId });
  }

  searchDockerImages(query: string): Promise<DockerSearchResult[]> {
    return invoke<DockerSearchResult[]>("search_docker_images", { query });
  }

  getLlmStatus(): Promise<LlmSnapshot> {
    return invoke<LlmSnapshot>("get_llm_status");
  }

  getLlmCatalog(): Promise<LlmCatalogEntry[]> {
    return invoke<LlmCatalogEntry[]>("get_llm_catalog");
  }

  installLlmEngine(): Promise<LlmSnapshot> {
    return invoke<LlmSnapshot>("install_llm_engine");
  }

  installModel(request: InstallModelRequest): Promise<LlmSnapshot> {
    return invoke<LlmSnapshot>("install_model", { request });
  }

  removeModel(modelId: string): Promise<LlmSnapshot> {
    return invoke<LlmSnapshot>("remove_model", { modelId });
  }

  startModel(modelId: string): Promise<LlmSnapshot> {
    return invoke<LlmSnapshot>("start_model", { modelId });
  }

  stopModel(): Promise<LlmSnapshot> {
    return invoke<LlmSnapshot>("stop_model");
  }

  copyText(text: string): Promise<void> {
    return invoke<void>("copy_text", { text });
  }

  hidePopup(): Promise<void> {
    return invoke<void>("hide_popup");
  }

  startDockerDesktop(): Promise<void> {
    return invoke<void>("start_docker_desktop");
  }

  resizePopup(height: number): Promise<void> {
    return invoke<void>("resize_popup", { height });
  }

  openMain(page: MainPage): Promise<void> {
    return invoke<void>("open_main", { page });
  }

  getMainPage(): Promise<MainPage> {
    return invoke<MainPage>("get_main_page");
  }

  getAutostartEnabled(): Promise<boolean> {
    return invoke<boolean>("get_autostart_enabled");
  }

  setAutostartEnabled(enabled: boolean): Promise<boolean> {
    return invoke<boolean>("set_autostart_enabled", { enabled });
  }

  onStatus(handler: (snapshot: ServicesSnapshot) => void): Promise<UnlistenFn> {
    return listen<ServicesSnapshot>("services-status", (event) =>
      handler(event.payload),
    );
  }

  onServiceError(
    handler: (payload: ServiceErrorPayload) => void,
  ): Promise<UnlistenFn> {
    return listen<ServiceErrorPayload>("service-error", (event) =>
      handler(event.payload),
    );
  }

  onToast(handler: (payload: ToastMessage) => void): Promise<UnlistenFn> {
    return listen<ToastMessage>("toast", (event) => handler(event.payload));
  }

  onSettings(handler: (settings: AppSettings) => void): Promise<UnlistenFn> {
    return listen<AppSettings>("settings-changed", (event) =>
      handler(event.payload),
    );
  }

  onMainPage(handler: (page: MainPage) => void): Promise<UnlistenFn> {
    return listen<MainPage>("open-main-page", (event) =>
      handler(event.payload),
    );
  }

  onLlmStatus(handler: (snapshot: LlmSnapshot) => void): Promise<UnlistenFn> {
    return listen<LlmSnapshot>("llm-status", (event) =>
      handler(event.payload),
    );
  }

  onLlmProgress(
    handler: (payload: LlmProgressPayload) => void,
  ): Promise<UnlistenFn> {
    return listen<LlmProgressPayload>("llm-download-progress", (event) =>
      handler(event.payload),
    );
  }
}
