import { Component, computed, inject } from "@angular/core";

import { DockletStore } from "../docklet.store";
import type { LlmModelStatus, ServiceStatus } from "../models";
import { ServiceIconComponent } from "../service-icon.component";

@Component({
  selector: "app-services-page",
  imports: [ServiceIconComponent],
  templateUrl: "./services-page.component.html",
  styleUrl: "./services-page.component.css",
})
export class ServicesPageComponent {
  readonly store = inject(DockletStore);
  readonly runningLabel = computed(
    () =>
      `${this.store.runningCount()} of ${this.store.services().length} running`,
  );

  stateLabel(service: ServiceStatus): string {
    const operation = this.store.operationFor(service.id);
    if (operation === "start") return "Starting";
    if (operation === "stop") return "Stopping";
    if (operation === "reset") return "Resetting";

    switch (service.state) {
      case "running":
        return "Running";
      case "starting":
        return "Starting";
      case "stopped":
        return "Stopped";
      case "missing":
        return "Not created";
      case "dockerUnavailable":
        return "Docker offline";
      case "unmanaged":
        return "Name conflict";
      case "error":
        return "Needs attention";
    }
  }

  toggle(service: ServiceStatus): void {
    void this.store.toggle(service);
  }

  copy(service: ServiceStatus): void {
    void this.store.copyService(service);
  }

  copyFromContext(event: Event, service: ServiceStatus): void {
    event.preventDefault();
    void this.store.copyService(service);
  }

  openSettings(): void {
    this.store.setMainPage("settings");
  }

  openModels(): void {
    this.store.setMainPage("models");
  }

  startDocker(): void {
    void this.store.startDockerDesktop();
  }

  toggleLlm(model: LlmModelStatus): void {
    void this.store.toggleLlm(model);
  }

  copyLlm(value: string | null | undefined, label: string): void {
    void this.store.copyLlmValue(value, label);
  }

  isLlmRunning(model: LlmModelStatus): boolean {
    return model.state === "running";
  }

  canToggleLlm(model: LlmModelStatus): boolean {
    return (
      (this.store.llmEngine()?.installed ?? false) &&
      !this.store.isLlmBusy(model.id) &&
      model.state !== "downloading" &&
      model.state !== "missing"
    );
  }

  llmStateLabel(model: LlmModelStatus): string {
    const operation = this.store.llmOperationFor(model.id);
    if (operation === "start") return "Starting";
    if (operation === "stop") return "Stopping";
    if (operation === "remove") return "Removing";

    switch (model.state) {
      case "running":
        return "Serving";
      case "downloading":
        return "Downloading";
      case "error":
        return "Needs attention";
      case "missing":
        return "File missing";
      default:
        return "Stopped";
    }
  }

  llmActionLabel(model: LlmModelStatus): string {
    const operation = this.store.llmOperationFor(model.id);
    if (operation === "start") return "Starting…";
    if (operation === "stop") return "Stopping…";
    return this.isLlmRunning(model) ? "Stop" : "Start";
  }

  llmProgress(model: LlmModelStatus): number {
    if (!model.totalBytes) {
      return 0;
    }
    return Math.min(
      100,
      Math.max(0, Math.round((model.downloadedBytes / model.totalBytes) * 100)),
    );
  }

  formatBytes(bytes: number): string {
    if (!bytes) {
      return "—";
    }
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit += 1;
    }
    return `${unit === 0 || value >= 10 ? Math.round(value) : value.toFixed(1)} ${
      units[unit]
    }`;
  }

  isRunning(service: ServiceStatus): boolean {
    return service.state === "running" || service.state === "starting";
  }

  actionLabel(service: ServiceStatus): string {
    if (this.store.isBusy(service.id)) {
      return "Working…";
    }
    return this.isRunning(service) ? "Stop" : "Start";
  }
}
