import {
  Component,
  DestroyRef,
  ElementRef,
  HostListener,
  computed,
  inject,
  signal,
  viewChild,
} from "@angular/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  RedisOperation,
  RedisStatus,
  StatusView,
  ToastMessage,
} from "./redis-status";
import { RedisService } from "./redis.service";

type PrimaryAction = "start" | "stop" | "retry";

@Component({
  selector: "app-root",
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent {
  private readonly redis = inject(RedisService);
  private readonly destroyRef = inject(DestroyRef);

  readonly status = signal<RedisStatus | null>(null);
  readonly loading = signal(true);
  readonly busy = signal<RedisOperation | null>(null);
  readonly error = signal<string | null>(null);
  readonly copying = signal(false);
  readonly autostartEnabled = signal(false);
  readonly autostartAvailable = signal(false);
  readonly autostartBusy = signal(false);
  readonly resetOpen = signal(false);
  readonly toast = signal<ToastMessage | null>(null);

  readonly cancelResetButton =
    viewChild<ElementRef<HTMLButtonElement>>("cancelResetButton");
  readonly confirmResetButton =
    viewChild<ElementRef<HTMLButtonElement>>("confirmResetButton");
  readonly resetTriggerButton =
    viewChild<ElementRef<HTMLButtonElement>>("resetTrigger");

  private pollHandle: ReturnType<typeof setInterval> | null = null;
  private toastHandle: ReturnType<typeof setTimeout> | null = null;
  private pollInFlight = false;
  private readonly unlisteners: UnlistenFn[] = [];
  private readonly domCleanup: Array<() => void> = [];

  readonly statusView = computed<StatusView>(() => {
    const busy = this.busy();
    if (busy === "start") {
      return {
        label: "Starting Redis",
        detail: "Creating the managed container and waiting for connections.",
        tone: "busy",
      };
    }
    if (busy === "stop") {
      return {
        label: "Stopping Redis",
        detail: "Saving data and shutting the container down gracefully.",
        tone: "busy",
      };
    }
    if (busy === "reset") {
      return {
        label: "Resetting Redis",
        detail: "Removing the managed volume and starting a clean instance.",
        tone: "busy",
      };
    }

    if (this.loading() && !this.status()) {
      return {
        label: "Checking Redis",
        detail: "Reading the local Docker state.",
        tone: "idle",
      };
    }

    const error = this.error();
    if (error) {
      return {
        label: "Couldn't finish that",
        detail: error,
        tone: "danger",
      };
    }

    const status = this.status();
    if (!status) {
      return {
        label: "Docker is unavailable",
        detail: "Start Docker Desktop, then try again.",
        tone: "warn",
      };
    }

    switch (status.state) {
      case "running":
        return {
          label: "Redis is running",
          detail: status.port
            ? `Managed container is ready on local port ${status.port}.`
            : "The managed container is online.",
          tone: "ok",
        };
      case "stopped":
        return {
          label: "Redis is stopped",
          detail: "The container and data volume are preserved.",
          tone: "idle",
        };
      case "missing":
        return {
          label: "Ready when you are",
          detail: "Start creates a managed container and data volume.",
          tone: "idle",
        };
      case "starting":
        return {
          label: "Redis is starting",
          detail: "Docker is bringing the managed container online.",
          tone: "busy",
        };
      case "dockerUnavailable":
        return {
          label: "Docker is offline",
          detail:
            status.error ??
            "Start Docker Desktop, then retry the connection.",
          tone: "warn",
        };
      case "unmanaged":
        return {
          label: "Container conflict",
          detail:
            status.error ??
            "A container named docklet-redis is not managed by Docklet.",
          tone: "danger",
        };
      case "error":
        return {
          label: "Redis needs attention",
          detail:
            status.error ?? "Check Docker, then try again.",
          tone: "danger",
        };
    }
  });

  readonly primaryAction = computed<PrimaryAction>(() => {
    if (this.error()) {
      return "retry";
    }

    const status = this.status();
    if (!status) {
      return "retry";
    }
    if (status.state === "running" || status.state === "starting") {
      return "stop";
    }
    if (status.state === "stopped" || status.state === "missing") {
      return "start";
    }
    return "retry";
  });

  readonly primaryLabel = computed(() => {
    const busy = this.busy();
    if (busy === "start") return "Starting Redis…";
    if (busy === "stop") return "Stopping Redis…";
    if (busy === "reset") return "Resetting Redis…";
    if (this.loading() && !this.status()) return "Checking Docker…";

    switch (this.primaryAction()) {
      case "start":
        return "Start Redis";
      case "stop":
        return "Stop Redis";
      case "retry":
        return "Retry connection";
    }
  });

  readonly primaryDisabled = computed(
    () => this.loading() || this.busy() !== null || this.resetOpen(),
  );

  readonly running = computed(
    () => this.status()?.state === "running",
  );

  readonly activePort = computed(() =>
    this.status()?.state === "running" ? this.status()?.port ?? null : null,
  );

  readonly canCopy = computed(() => {
    const status = this.status();
    return (
      status?.state === "running" &&
      Boolean(status.url) &&
      !this.busy() &&
      !this.copying() &&
      !this.resetOpen()
    );
  });

  readonly canResetData = computed(() => {
    const status = this.status();
    if (!status || !status.managed || !status.containerState) {
      return false;
    }
    return (
      ["running", "stopped", "error"].includes(status.state) &&
      !this.busy() &&
      !this.loading()
    );
  });

  readonly canReset = computed(
    () => this.canResetData() && !this.resetOpen(),
  );

  readonly displayUrl = computed(() => {
    const status = this.status();
    if (status?.url) {
      return status.url;
    }
    if (status?.state === "missing") {
      return "Not created yet";
    }
    if (status?.state === "dockerUnavailable") {
      return "Docker unavailable";
    }
    if (!status && this.loading()) {
      return "Checking…";
    }
    return "No published port";
  });

  readonly connectionHint = computed(() => {
    switch (this.status()?.state) {
      case "running":
        return "Ready to connect";
      case "stopped":
        return "Start to connect";
      case "missing":
        return "Created on first start";
      case "dockerUnavailable":
        return "Waiting for Docker";
      case "unmanaged":
        return "Name conflict";
      default:
        return "Unavailable";
    }
  });

  readonly utilityNote = computed(() => {
    switch (this.status()?.state) {
      case "running":
        return "Stop preserves your data.";
      case "stopped":
        return "Reset clears the volume.";
      case "missing":
        return "Managed locally.";
      case "dockerUnavailable":
        return "Docker Desktop required.";
      case "unmanaged":
        return "Not Docklet-managed.";
      default:
        return "Check Docker, then retry.";
    }
  });

  readonly dockerLabel = computed(() => {
    const status = this.status();
    if (!status) {
      return this.loading() ? "Checking Docker" : "Docker unknown";
    }
    return status.dockerAvailable ? "Docker ready" : "Docker offline";
  });

  readonly dockerOnline = computed(
    () => this.status()?.dockerAvailable === true,
  );

  readonly autostartTitle = computed(() =>
    this.autostartAvailable()
      ? "Launch Docklet when you sign in"
      : "Launch-at-login is unavailable in this environment",
  );

  constructor() {
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

  ngOnInit(): void {
    this.bindDesktopEvents();
    void this.registerBackendEvents();
    void this.bootstrap();
  }

  @HostListener("document:keydown.escape")
  onEscape(): void {
    if (this.resetOpen()) {
      this.closeReset();
      return;
    }
    this.hide();
  }

  onSheetKeydown(event: KeyboardEvent): void {
    if (event.key !== "Tab") {
      return;
    }

    const first = this.cancelResetButton()?.nativeElement;
    const last = this.confirmResetButton()?.nativeElement;
    if (!first || !last) {
      return;
    }

    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  async onPrimary(): Promise<void> {
    switch (this.primaryAction()) {
      case "start":
        await this.startRedis();
        break;
      case "stop":
        await this.stopRedis();
        break;
      case "retry":
        await this.retry();
        break;
    }
  }

  async copyUrl(): Promise<void> {
    if (!this.canCopy()) {
      return;
    }

    this.copying.set(true);
    try {
      await this.redis.copyUrl();
      this.showToast("Connection URL copied");
    } catch (error) {
      this.showToast(this.describeError(error), "danger");
    } finally {
      this.copying.set(false);
    }
  }

  openReset(): void {
    if (!this.canReset()) {
      return;
    }
    this.error.set(null);
    this.resetOpen.set(true);
    setTimeout(() => this.cancelResetButton()?.nativeElement.focus(), 0);
  }

  closeReset(): void {
    if (!this.resetOpen()) {
      return;
    }
    this.resetOpen.set(false);
    setTimeout(() => this.resetTriggerButton()?.nativeElement.focus(), 0);
  }

  async confirmReset(): Promise<void> {
    if (!this.canResetData()) {
      this.closeReset();
      return;
    }

    this.resetOpen.set(false);
    this.error.set(null);
    this.busy.set("reset");

    try {
      const status = await this.redis.reset();
      this.status.set(status);
      this.showToast("Redis reset · fresh volume ready");
    } catch (error) {
      this.error.set(this.describeError(error));
      await this.refreshQuietly();
    } finally {
      this.busy.set(null);
    }
  }

  async toggleAutostart(): Promise<void> {
    if (!this.autostartAvailable() || this.autostartBusy()) {
      return;
    }

    const next = !this.autostartEnabled();
    this.autostartBusy.set(true);
    try {
      const enabled = await this.redis.setAutostartEnabled(next);
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

  hide(): void {
    void this.redis.hidePopup().catch(() => undefined);
  }

  private async bootstrap(): Promise<void> {
    const [statusResult, autostartResult] = await Promise.allSettled([
      this.redis.getStatus(),
      this.redis.getAutostartEnabled(),
    ]);

    if (statusResult.status === "fulfilled") {
      this.status.set(statusResult.value);
    } else {
      this.error.set(this.describeError(statusResult.reason));
    }
    this.loading.set(false);

    if (autostartResult.status === "fulfilled") {
      this.autostartEnabled.set(autostartResult.value);
      this.autostartAvailable.set(true);
    }

    if (document.hasFocus() && !document.hidden) {
      this.startPolling();
    }
  }

  private async registerBackendEvents(): Promise<void> {
    try {
      const listeners = await Promise.all([
        this.redis.onStatus((status) => {
          this.status.set(status);
          this.loading.set(false);
        }),
        this.redis.onError((message) => {
          this.error.set(message);
          void this.refreshQuietly();
        }),
        this.redis.onToast((message) => {
          if (!document.hidden) {
            this.showToast(message);
          }
        }),
      ]);
      this.unlisteners.push(...listeners);
    } catch {
      // The desktop bridge may not be available in a plain browser preview.
    }
  }

  private bindDesktopEvents(): void {
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
    this.pollHandle = setInterval(() => void this.pollStatus(), 2000);
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
      this.busy() ||
      this.resetOpen() ||
      document.hidden
    ) {
      return;
    }

    this.pollInFlight = true;
    try {
      const status = await this.redis.getStatus();
      this.status.set(status);
      this.loading.set(false);
    } catch {
      // Keep the last good status; explicit actions surface errors.
    } finally {
      this.pollInFlight = false;
    }
  }

  private async startRedis(): Promise<void> {
    this.error.set(null);
    this.busy.set("start");
    try {
      const status = await this.redis.start();
      this.status.set(status);
      this.showToast("Redis is ready");
    } catch (error) {
      this.error.set(this.describeError(error));
      await this.refreshQuietly();
    } finally {
      this.busy.set(null);
    }
  }

  private async stopRedis(): Promise<void> {
    this.error.set(null);
    this.busy.set("stop");
    try {
      const status = await this.redis.stop();
      this.status.set(status);
      this.showToast("Redis stopped · data preserved");
    } catch (error) {
      this.error.set(this.describeError(error));
      await this.refreshQuietly();
    } finally {
      this.busy.set(null);
    }
  }

  private async retry(): Promise<void> {
    this.error.set(null);
    this.loading.set(true);
    try {
      const status = await this.redis.getStatus();
      this.status.set(status);
    } catch (error) {
      this.error.set(this.describeError(error));
    } finally {
      this.loading.set(false);
    }
  }

  private async refreshQuietly(): Promise<void> {
    try {
      const status = await this.redis.getStatus();
      this.status.set(status);
      this.loading.set(false);
    } catch {
      // Keep the existing state when a background refresh fails.
    }
  }

  private showToast(
    message: string,
    tone: ToastMessage["tone"] = "default",
  ): void {
    if (this.toastHandle !== null) {
      clearTimeout(this.toastHandle);
    }
    this.toast.set({ message, tone });
    this.toastHandle = setTimeout(
      () => this.toast.set(null),
      tone === "danger" ? 3600 : 2400,
    );
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
