import {
  DestroyRef,
  Injectable,
  computed,
  inject,
  signal,
} from "@angular/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { DockletApi } from "./docklet-api.service";
import type {
  AppSettings,
  MainPage,
  ServiceOperation,
  ServiceStatus,
  ServicesSnapshot,
  ToastMessage,
  ToastTone,
} from "./models";

type WindowRole = "popup" | "main";

@Injectable({ providedIn: "root" })
export class DockletStore {
  private readonly api = inject(DockletApi);
  private readonly destroyRef = inject(DestroyRef);

  readonly role = signal<WindowRole>("popup");
  readonly snapshot = signal<ServicesSnapshot | null>(null);
  readonly settings = signal<AppSettings | null>(null);
  readonly loading = signal(true);
  readonly busy = signal<Record<string, ServiceOperation>>({});
  readonly toast = signal<ToastMessage | null>(null);
  readonly serviceErrors = signal<Record<string, string>>({});
  readonly settingsSaving = signal(false);
  readonly autostartEnabled = signal(false);
  readonly autostartAvailable = signal(false);
  readonly autostartBusy = signal(false);
  readonly mainPage = signal<MainPage>("services");

  readonly services = computed<ServiceStatus[]>(
    () => this.snapshot()?.services ?? [],
  );
  readonly runningCount = computed(
    () =>
      this.services().filter((service) => service.state === "running").length,
  );
  readonly startingCount = computed(
    () =>
      this.services().filter((service) => service.state === "starting").length,
  );
  readonly dockerAvailable = computed(
    () => this.snapshot()?.dockerAvailable ?? false,
  );

  private initialized = false;
  private pollHandle: ReturnType<typeof setInterval> | null = null;
  private pollInFlight = false;
  private toastHandle: ReturnType<typeof setTimeout> | null = null;
  private readonly unlisteners: UnlistenFn[] = [];
  private readonly domCleanup: Array<() => void> = [];

  init(role: WindowRole): void {
    if (this.initialized) {
      return;
    }
    this.initialized = true;
    this.role.set(role);

    this.bindWindowEvents();
    void this.registerBackendEvents();
    void this.bootstrap();

    this.destroyRef.onDestroy(() => {
      this.stopPolling();
      if (this.toastHandle !== null) {
        clearTimeout(this.toastHandle);
      }
      for (const cleanup of this.domCleanup) {
        cleanup();
      }
      for (const unlisten of this.unlisteners) {
        unlisten();
      }
    });
  }

  service(id: string): ServiceStatus | undefined {
    return this.services().find((service) => service.id === id);
  }

  operationFor(id: string): ServiceOperation | null {
    return this.busy()[id] ?? null;
  }

  isBusy(id: string): boolean {
    return this.operationFor(id) !== null;
  }

  async toggle(service: ServiceStatus): Promise<void> {
    if (this.isBusy(service.id)) {
      return;
    }
    if (service.state === "running" || service.state === "starting") {
      await this.stopService(service);
    } else {
      await this.startService(service);
    }
  }

  async startService(service: ServiceStatus): Promise<void> {
    await this.runOperation(service, "start", `${service.name} is ready`);
  }

  async stopService(service: ServiceStatus): Promise<void> {
    await this.runOperation(
      service,
      "stop",
      `${service.name} stopped · data preserved`,
    );
  }

  async resetService(service: ServiceStatus): Promise<void> {
    await this.runOperation(
      service,
      "reset",
      `${service.name} reset · clean data volume ready`,
    );
  }

  async copyService(service: ServiceStatus): Promise<void> {
    if (service.state !== "running" || !service.url) {
      this.showToast(`Start ${service.name} before copying its URL.`, "danger");
      return;
    }

    try {
      await this.api.copyServiceUrl(service.id);
      this.showToast(`${service.name} URL copied`);
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
    }
  }

  async openMain(page: MainPage): Promise<void> {
    try {
      await this.api.openMain(page);
      this.mainPage.set(page);
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
    }
  }

  hidePopup(): void {
    void this.api.hidePopup().catch(() => undefined);
  }

  setMainPage(page: MainPage): void {
    this.mainPage.set(page);
  }

  async toggleAutostart(): Promise<void> {
    if (!this.autostartAvailable() || this.autostartBusy()) {
      return;
    }

    const next = !this.autostartEnabled();
    this.autostartBusy.set(true);
    try {
      const enabled = await this.api.setAutostartEnabled(next);
      this.autostartEnabled.set(enabled);
      this.showToast(
        enabled ? "Docklet will launch at login" : "Launch at login disabled",
      );
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
    } finally {
      this.autostartBusy.set(false);
    }
  }

  async saveSettings(settings: AppSettings): Promise<boolean> {
    if (this.settingsSaving()) {
      return false;
    }

    this.settingsSaving.set(true);
    try {
      const saved = await this.api.saveSettings(settings);
      this.settings.set(saved);
      this.showToast("Settings saved");
      await this.refreshQuietly();
      return true;
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      return false;
    } finally {
      this.settingsSaving.set(false);
    }
  }

  async removeCustomService(service: ServiceStatus): Promise<boolean> {
    try {
      const settings = await this.api.removeCustomService(service.id);
      this.settings.set(settings);
      this.showToast(`${service.name} removed`);
      await this.refreshQuietly();
      return true;
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      return false;
    }
  }

  refresh(): Promise<void> {
    return this.refreshQuietly(true);
  }

  showToast(message: string, tone: ToastTone = "default"): void {
    if (this.toastHandle !== null) {
      clearTimeout(this.toastHandle);
    }
    this.toast.set({ message, tone });
    this.toastHandle = setTimeout(
      () => this.toast.set(null),
      tone === "danger" ? 4200 : 2600,
    );
  }

  private async runOperation(
    service: ServiceStatus,
    operation: ServiceOperation,
    successMessage: string,
  ): Promise<void> {
    if (this.isBusy(service.id)) {
      return;
    }

    this.setBusy(service.id, operation);
    try {
      const snapshot =
        operation === "start"
          ? await this.api.startService(service.id)
          : operation === "stop"
            ? await this.api.stopService(service.id)
            : await this.api.resetService(service.id);
      this.snapshot.set(snapshot);
      this.loading.set(false);
      this.showToast(successMessage);
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      await this.refreshQuietly();
    } finally {
      this.setBusy(service.id, null);
    }
  }

  private setBusy(id: string, operation: ServiceOperation | null): void {
    this.busy.update((current) => {
      const next = { ...current };
      if (operation === null) {
        delete next[id];
      } else {
        next[id] = operation;
      }
      return next;
    });
  }

  private async bootstrap(): Promise<void> {
    const tasks: Array<Promise<unknown>> = [
      this.api.getServices(),
      this.api.getAutostartEnabled(),
    ];
    if (this.role() === "main") {
      tasks.push(this.api.getSettings(), this.api.getMainPage());
    }

    const [servicesResult, autostartResult, settingsResult, pageResult] =
      await Promise.allSettled(tasks);

    if (servicesResult?.status === "fulfilled") {
      this.snapshot.set(servicesResult.value as ServicesSnapshot);
      this.loading.set(false);
    } else if (servicesResult?.status === "rejected") {
      this.loading.set(false);
      this.showToast(this.describeError(servicesResult.reason), "danger");
    }

    if (autostartResult?.status === "fulfilled") {
      this.autostartEnabled.set(autostartResult.value as boolean);
      this.autostartAvailable.set(true);
    }

    if (settingsResult?.status === "fulfilled") {
      this.settings.set(settingsResult.value as AppSettings);
    }

    if (pageResult?.status === "fulfilled") {
      const page = pageResult.value as MainPage;
      this.mainPage.set(page === "settings" ? "settings" : "services");
    }

    if (document.hasFocus() && !document.hidden) {
      this.startPolling();
    }
  }

  private async registerBackendEvents(): Promise<void> {
    try {
      const listeners = await Promise.all([
        this.api.onStatus((snapshot) => {
          this.snapshot.set(snapshot);
          this.loading.set(false);
        }),
        this.api.onServiceError((payload) => {
          this.serviceErrors.update((current) => ({
            ...current,
            [payload.serviceId]: payload.message,
          }));
        }),
        this.api.onToast((message) => {
          if (!document.hidden) {
            this.showToast(message.message, message.tone);
          }
        }),
        this.api.onSettings((settings) => {
          this.settings.set(settings);
        }),
        this.api.onMainPage((page) => {
          this.mainPage.set(page === "settings" ? "settings" : "services");
        }),
      ]);
      this.unlisteners.push(...listeners);
    } catch {
      // The desktop bridge is unavailable in a plain browser preview.
    }
  }

  private bindWindowEvents(): void {
    const onFocus = () => {
      void this.refreshQuietly();
      this.startPolling();
    };
    const onBlur = () => this.stopPolling();
    const onVisibility = () => {
      if (document.hidden) {
        this.stopPolling();
      } else {
        void this.refreshQuietly();
        this.startPolling();
      }
    };

    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", onVisibility);
    this.domCleanup.push(
      () => window.removeEventListener("focus", onFocus),
      () => window.removeEventListener("blur", onBlur),
      () => document.removeEventListener("visibilitychange", onVisibility),
    );
  }

  private startPolling(): void {
    if (this.pollHandle !== null) {
      return;
    }
    this.pollHandle = setInterval(() => void this.pollStatus(), 3000);
    void this.pollStatus();
  }

  private stopPolling(): void {
    if (this.pollHandle !== null) {
      clearInterval(this.pollHandle);
      this.pollHandle = null;
    }
  }

  private async pollStatus(): Promise<void> {
    if (
      this.pollInFlight ||
      Object.keys(this.busy()).length > 0 ||
      document.hidden
    ) {
      return;
    }

    this.pollInFlight = true;
    try {
      const snapshot = await this.api.getServices();
      this.snapshot.set(snapshot);
      this.loading.set(false);
    } catch {
      // Keep the last good snapshot; explicit actions surface errors.
    } finally {
      this.pollInFlight = false;
    }
  }

  private async refreshQuietly(showError = false): Promise<void> {
    try {
      const snapshot = await this.api.getServices();
      this.snapshot.set(snapshot);
      this.loading.set(false);
    } catch (error) {
      if (showError) {
        this.showToast(this.describeError(error), "danger");
      }
    }
  }

  private describeError(error: unknown): string {
    if (typeof error === "string") {
      return error;
    }
    if (error instanceof Error) {
      return error.message;
    }
    try {
      return JSON.stringify(error);
    } catch {
      return "Something unexpected happened. Try again.";
    }
  }
}
