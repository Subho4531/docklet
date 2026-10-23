import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppSettings,
  DockerSearchResult,
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

  hidePopup(): Promise<void> {
    return invoke<void>("hide_popup");
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
}
