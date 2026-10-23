import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { RedisStatus } from "./redis-status";

@Injectable({ providedIn: "root" })
export class RedisService {
  getStatus(): Promise<RedisStatus> {
    return invoke<RedisStatus>("get_redis_status");
  }

  start(): Promise<RedisStatus> {
    return invoke<RedisStatus>("start_redis");
  }

  stop(): Promise<RedisStatus> {
    return invoke<RedisStatus>("stop_redis");
  }

  reset(): Promise<RedisStatus> {
    return invoke<RedisStatus>("reset_redis");
  }

  copyUrl(): Promise<string> {
    return invoke<string>("copy_redis_url");
  }

  hidePopup(): Promise<void> {
    return invoke<void>("hide_popup");
  }

  getAutostartEnabled(): Promise<boolean> {
    return invoke<boolean>("get_autostart_enabled");
  }

  setAutostartEnabled(enabled: boolean): Promise<boolean> {
    return invoke<boolean>("set_autostart_enabled", { enabled });
  }

  onStatus(handler: (status: RedisStatus) => void): Promise<UnlistenFn> {
    return listen<RedisStatus>("redis-status", (event) => handler(event.payload));
  }

  onError(handler: (message: string) => void): Promise<UnlistenFn> {
    return listen<string>("redis-error", (event) => handler(event.payload));
  }

  onToast(handler: (message: string) => void): Promise<UnlistenFn> {
    return listen<string>("redis-toast", (event) => handler(event.payload));
  }
}
