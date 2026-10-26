import { Component, computed, inject, signal } from "@angular/core";

import { DockletStore } from "../docklet.store";
import type { LlmCatalogEntry, LlmFit, LlmModelStatus } from "../models";

@Component({
  selector: "app-models-page",
  templateUrl: "./models-page.component.html",
  styleUrl: "./models-page.component.css",
})
export class ModelsPageComponent {
  readonly store = inject(DockletStore);
  readonly customOpen = signal(false);
  readonly customRepo = signal("");
  readonly customFile = signal("");
  readonly customName = signal("");
  readonly pendingRemove = signal<string | null>(null);

  readonly engineReady = computed(
    () => this.store.llmEngine()?.installed ?? false,
  );

  isInstalled(id: string): boolean {
    return this.store.llmModels().some((model) => model.id === id);
  }

  entryModel(id: string): LlmModelStatus | undefined {
    return this.store.llmModels().find((model) => model.id === id);
  }

  isDownloading(entry: LlmCatalogEntry): boolean {
    return this.entryModel(entry.id)?.state === "downloading";
  }

  installEngine(): void {
    void this.store.installEngine();
  }

  install(entry: LlmCatalogEntry): void {
    if (!this.engineReady() || this.isInstalled(entry.id) || this.isDownloading(entry)) {
      return;
    }
    void this.store.installCatalogModel(entry);
  }

  toggleCustom(): void {
    const open = !this.customOpen();
    this.customOpen.set(open);
    if (!open) {
      this.clearCustom();
    }
  }

  async installCustom(event: Event): Promise<void> {
    event.preventDefault();
    const repo = this.customRepo().trim();
    const file = this.customFile().trim();
    const name = this.customName().trim();

    if (!repo || !file) {
      this.store.showToast(
        "Enter a Hugging Face repo and a GGUF file name.",
        "danger",
      );
      return;
    }
    if (!this.engineReady()) {
      this.store.showToast("Install the llama.cpp engine first.", "danger");
      return;
    }

    const installed = await this.store.installCustomModel(repo, file, name);
    if (installed) {
      this.customOpen.set(false);
      this.clearCustom();
    }
  }

  toggle(model: LlmModelStatus): void {
    void this.store.toggleLlm(model);
  }

  copy(value: string | null | undefined, label: string): void {
    void this.store.copyLlmValue(value, label);
  }

  requestRemove(id: string): void {
    this.pendingRemove.set(id);
  }

  cancelRemove(): void {
    this.pendingRemove.set(null);
  }

  async confirmRemove(model: LlmModelStatus): Promise<void> {
    this.pendingRemove.set(null);
    await this.store.removeLlm(model);
  }

  canToggle(model: LlmModelStatus): boolean {
    return (
      this.engineReady() &&
      !this.isLlmBusy(model) &&
      model.state !== "downloading" &&
      model.state !== "missing"
    );
  }

  isLlmBusy(model: LlmModelStatus): boolean {
    return this.store.isLlmBusy(model.id);
  }

  actionLabel(model: LlmModelStatus): string {
    const operation = this.store.llmOperationFor(model.id);
    if (operation === "start") return "Starting…";
    if (operation === "stop") return "Stopping…";
    if (operation === "remove") return "Removing…";
    return model.state === "running" ? "Stop" : "Start";
  }

  stateLabel(model: LlmModelStatus): string {
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

  fitLabel(fit: LlmFit): string {
    if (fit === "comfortable") return "Fits comfortably";
    if (fit === "tight") return "Fits tightly";
    return "CPU heavy";
  }

  formatContext(tokens: number): string {
    if (tokens >= 1024) {
      const value = tokens / 1024;
      return `${Number.isInteger(value) ? value : value.toFixed(1)}K ctx`;
    }
    return `${tokens} ctx`;
  }

  formatSize(bytes: number): string {
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

  progressPercent(downloaded: number, total: number): number {
    if (!total) {
      return 0;
    }
    return Math.min(100, Math.max(0, Math.round((downloaded / total) * 100)));
  }

  private clearCustom(): void {
    this.customRepo.set("");
    this.customFile.set("");
    this.customName.set("");
  }
}
