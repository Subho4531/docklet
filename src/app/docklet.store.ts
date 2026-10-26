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
  CustomService,
  DockerSearchResult,
  InstallModelRequest,
  LlmCatalogEntry,
  LlmModelStatus,
  LlmOperation,
  LlmProgressPayload,
  LlmSnapshot,
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
  readonly llm = signal<LlmSnapshot | null>(null);
  readonly llmCatalog = signal<LlmCatalogEntry[]>([]);
  readonly llmBusy = signal<Record<string, LlmOperation>>({});
  readonly engineBusy = signal(false);

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
  readonly llmModels = computed<LlmModelStatus[]>(
    () => this.llm()?.models ?? [],
  );
  readonly llmEngine = computed(() => this.llm()?.engine ?? null);
  readonly llmRunningCount = computed(
    () => this.llmModels().filter((model) => model.state === "running").length,
  );
  readonly llmInstalledCount = computed(
    () =>
      this.llmModels().filter((model) => model.state !== "downloading").length,
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

  searchDockerImages(query: string): Promise<DockerSearchResult[]> {
    return this.api.searchDockerImages(query);
  }

  resizePopup(height: number): Promise<void> {
    return this.api.resizePopup(height);
  }

  async addCustomService(service: CustomService): Promise<boolean> {
    if (this.settingsSaving()) {
      return false;
    }

    this.settingsSaving.set(true);
    try {
      let settings = this.settings();
      if (!settings) {
        settings = await this.api.getSettings();
      }
      const saved = await this.api.saveSettings({
        ...settings,
        customServices: [...settings.customServices, service],
      });
      this.settings.set(saved);
      this.showToast(`${service.name} added to Docklet`);
      await this.refreshQuietly();
      return true;
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      return false;
    } finally {
      this.settingsSaving.set(false);
    }
  }

  async startServiceById(serviceId: string): Promise<void> {
    let service = this.service(serviceId);
    if (!service) {
      await this.refreshQuietly();
      service = this.service(serviceId);
    }
    if (!service) {
      this.showToast("The new service couldn't be loaded. Open Services and try again.", "danger");
      return;
    }
    await this.startService(service);
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

  llmModel(id: string): LlmModelStatus | undefined {
    return this.llmModels().find((model) => model.id === id);
  }

  llmOperationFor(id: string): LlmOperation | null {
    return this.llmBusy()[id] ?? null;
  }

  isLlmBusy(id: string): boolean {
    return this.llmOperationFor(id) !== null;
  }

  async installEngine(): Promise<void> {
    if (this.engineBusy()) {
      return;
    }

    this.engineBusy.set(true);
    try {
      const snapshot = await this.api.installLlmEngine();
      this.llm.set(snapshot);
      this.showToast("llama.cpp engine ready");
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      await this.refreshLlmQuietly();
    } finally {
      this.engineBusy.set(false);
    }
  }

  installCatalogModel(entry: LlmCatalogEntry): Promise<boolean> {
    return this.installModel(
      { catalogId: entry.id },
      entry.id,
      entry.displayName,
    );
  }

  installCustomModel(
    repo: string,
    fileName: string,
    displayName: string,
  ): Promise<boolean> {
    return this.installModel(
      {
        repo,
        fileName,
        displayName: displayName || null,
      },
      "custom",
      displayName || fileName,
    );
  }

  toggleLlm(model: LlmModelStatus): Promise<void> {
    if (model.state === "running") {
      return this.stopLlm(model);
    }
    return this.startLlm(model);
  }

  async removeLlm(model: LlmModelStatus): Promise<boolean> {
    if (this.isLlmBusy(model.id)) {
      return false;
    }

    this.setLlmBusy(model.id, "remove");
    try {
      const snapshot = await this.api.removeModel(model.id);
      this.llm.set(snapshot);
      this.showToast(`${model.displayName} removed from Docklet`);
      return true;
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      await this.refreshLlmQuietly();
      return false;
    } finally {
      this.setLlmBusy(model.id, null);
    }
  }

  async copyLlmValue(
    value: string | null | undefined,
    label: string,
  ): Promise<void> {
    if (!value) {
      this.showToast(`No ${label.toLowerCase()} is available yet.`, "danger");
      return;
    }
    try {
      await this.api.copyText(value);
      this.showToast(`${label} copied`);
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
    }
  }

  private async installModel(
    request: InstallModelRequest,
    busyKey: string,
    label: string,
  ): Promise<boolean> {
    if (this.llmBusy()[busyKey]) {
      return false;
    }

    this.setLlmBusy(busyKey, "install");
    try {
      const snapshot = await this.api.installModel(request);
      this.llm.set(snapshot);
      this.showToast(`${label} installed`);
      return true;
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      await this.refreshLlmQuietly();
      return false;
    } finally {
      this.setLlmBusy(busyKey, null);
    }
  }

  private async startLlm(model: LlmModelStatus): Promise<void> {
    if (this.isLlmBusy(model.id)) {
      return;
    }

    this.setLlmBusy(model.id, "start");
    try {
      const snapshot = await this.api.startModel(model.id);
      this.llm.set(snapshot);
      this.showToast(`${model.displayName} is ready`);
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      await this.refreshLlmQuietly();
    } finally {
      this.setLlmBusy(model.id, null);
    }
  }

  private async stopLlm(model: LlmModelStatus): Promise<void> {
    if (this.isLlmBusy(model.id)) {
      return;
    }

    this.setLlmBusy(model.id, "stop");
    try {
      const snapshot = await this.api.stopModel();
      this.llm.set(snapshot);
      this.showToast(`${model.displayName} stopped`);
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
      await this.refreshLlmQuietly();
    } finally {
      this.setLlmBusy(model.id, null);
    }
  }

  private setLlmBusy(id: string, operation: LlmOperation | null): void {
    this.llmBusy.update((current) => {
      const next = { ...current };
      if (operation === null) {
        delete next[id];
      } else {
        next[id] = operation;
      }
      return next;
    });
  }

  private async refreshLlmQuietly(): Promise<void> {
    try {
      this.llm.set(await this.api.getLlmStatus());
    } catch {
      // Keep the last known snapshot; explicit actions surface errors.
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
      tasks.push(
        this.api.getSettings(),
        this.api.getMainPage(),
        this.api.getLlmCatalog(),
        this.api.getLlmStatus(),
      );
    }

    const results = await Promise.allSettled(tasks);
    const servicesResult = results[0];
    const autostartResult = results[1];
    const settingsResult = results[2];
    const pageResult = results[3];
    const catalogResult = results[4];
    const llmResult = results[5];

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
      this.mainPage.set(this.normalizePage(pageResult.value));
    }

    if (catalogResult?.status === "fulfilled") {
      this.llmCatalog.set(catalogResult.value as LlmCatalogEntry[]);
    }

    if (llmResult?.status === "fulfilled") {
      this.llm.set(llmResult.value as LlmSnapshot);
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
          this.mainPage.set(this.normalizePage(page));
        }),
        this.api.onLlmStatus((snapshot) => {
          this.llm.set(snapshot);
        }),
        this.api.onLlmProgress((payload) => {
          this.applyLlmProgress(payload);
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
      if (this.role() === "main") {
        await this.refreshLlmQuietly();
      }
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
    if (this.role() === "main") {
      await this.refreshLlmQuietly();
    }
  }

  private applyLlmProgress(payload: LlmProgressPayload): void {
    this.llm.update((snapshot) => {
      if (!snapshot) {
        return snapshot;
      }
      if (payload.id === "engine") {
        return {
          ...snapshot,
          engine: {
            ...snapshot.engine,
            downloadedBytes: payload.downloadedBytes,
            totalBytes: payload.totalBytes,
          },
        };
      }
      return {
        ...snapshot,
        models: snapshot.models.map((model) =>
          model.id === payload.id
            ? {
                ...model,
                downloadedBytes: payload.downloadedBytes,
                totalBytes: payload.totalBytes,
              }
            : model,
        ),
      };
    });
  }

  private normalizePage(page: unknown): MainPage {
    return page === "models" || page === "settings" || page === "library"
      ? page
      : "services";
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
