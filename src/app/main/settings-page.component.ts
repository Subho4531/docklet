import { Component, computed, effect, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";

import { ConfirmDialogComponent } from "../confirm-dialog.component";
import { DockletStore } from "../docklet.store";
import type { AppSettings, CustomService, ServiceStatus } from "../models";
import { ServiceIconComponent } from "../service-icon.component";
import { CustomServiceDialogComponent } from "./custom-service-dialog.component";

@Component({
  selector: "app-settings-page",
  imports: [
    FormsModule,
    ServiceIconComponent,
    CustomServiceDialogComponent,
    ConfirmDialogComponent,
  ],
  templateUrl: "./settings-page.component.html",
  styleUrl: "./settings-page.component.css",
})
export class SettingsPageComponent {
  readonly store = inject(DockletStore);

  readonly draft = signal<AppSettings | null>(null);
  readonly dirty = signal(false);
  readonly customDialogOpen = signal(false);
  readonly editingCustom = signal<CustomService | null>(null);
  readonly pendingReset = signal<ServiceStatus | null>(null);
  readonly pendingRemove = signal<ServiceStatus | null>(null);

  readonly builtinServices = computed(() =>
    this.store.services().filter((service) => service.kind === "builtin"),
  );
  readonly existingCustomIds = computed(
    () => this.draft()?.customServices.map((service) => service.id) ?? [],
  );
  readonly customRows = computed(() => {
    const statuses = new Map(
      this.store.services().map((service) => [service.id, service]),
    );
    return (this.draft()?.customServices ?? []).map((custom) => ({
      custom,
      status: statuses.get(custom.id),
    }));
  });

  constructor() {
    effect(() => {
      const settings = this.store.settings();
      const dirty = this.dirty();
      if (settings && !dirty) {
        this.draft.set(structuredClone(settings));
      }
    });
  }

  portFor(service: ServiceStatus): number {
    return this.draft()?.servicePorts[service.id] ?? service.hostPort;
  }

  setPort(id: string, value: number | string | null): void {
    const port = Number(value);
    this.draft.update((settings) =>
      settings
        ? {
            ...settings,
            servicePorts: { ...settings.servicePorts, [id]: port },
          }
        : settings,
    );
    this.dirty.set(true);
  }

  toggleHidePopup(): void {
    this.draft.update((settings) =>
      settings
        ? { ...settings, hidePopupOnBlur: !settings.hidePopupOnBlur }
        : settings,
    );
    this.dirty.set(true);
  }

  async save(): Promise<void> {
    const settings = this.draft();
    if (!settings || !this.dirty()) {
      return;
    }
    const saved = await this.store.saveSettings(settings);
    if (saved) {
      this.dirty.set(false);
      const latest = this.store.settings();
      if (latest) {
        this.draft.set(structuredClone(latest));
      }
    }
  }

  discard(): void {
    const settings = this.store.settings();
    if (settings) {
      this.draft.set(structuredClone(settings));
      this.dirty.set(false);
    }
  }

  portChanged(service: ServiceStatus): boolean {
    return service.port !== null && service.port !== this.portFor(service);
  }

  canReset(service: ServiceStatus): boolean {
    return (
      !this.store.isBusy(service.id) &&
      service.managed &&
      !["missing", "dockerUnavailable", "unmanaged"].includes(service.state)
    );
  }

  requestReset(service: ServiceStatus): void {
    if (this.canReset(service)) {
      this.pendingReset.set(service);
    }
  }

  async confirmReset(): Promise<void> {
    const service = this.pendingReset();
    this.pendingReset.set(null);
    if (service) {
      await this.store.resetService(service);
    }
  }

  requestRemove(service: ServiceStatus | undefined): void {
    if (!service) {
      return;
    }
    if (this.dirty()) {
      this.store.showToast(
        "Save or discard your settings changes before removing a service.",
        "danger",
      );
      return;
    }
    this.pendingRemove.set(service);
  }

  async confirmRemove(): Promise<void> {
    const service = this.pendingRemove();
    this.pendingRemove.set(null);
    if (!service) {
      return;
    }
    const removed = await this.store.removeCustomService(service);
    if (removed) {
      this.dirty.set(false);
      const latest = this.store.settings();
      if (latest) {
        this.draft.set(structuredClone(latest));
      }
    }
  }

  openAddCustom(): void {
    this.editingCustom.set(null);
    this.customDialogOpen.set(true);
  }

  openEditCustom(custom: CustomService): void {
    this.editingCustom.set(structuredClone(custom));
    this.customDialogOpen.set(true);
  }

  closeCustomDialog(): void {
    this.customDialogOpen.set(false);
    this.editingCustom.set(null);
  }

  saveCustom(custom: CustomService): void {
    this.draft.update((settings) => {
      if (!settings) {
        return settings;
      }
      const services = [...settings.customServices];
      const index = services.findIndex((service) => service.id === custom.id);
      if (index >= 0) {
        services[index] = custom;
      } else {
        services.push(custom);
      }
      return { ...settings, customServices: services };
    });
    this.dirty.set(true);
    this.closeCustomDialog();
  }

  stateLabel(service: ServiceStatus | undefined): string {
    if (!service) {
      return "Not saved";
    }
    if (this.store.isBusy(service.id)) {
      return "Working…";
    }
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

  customPortLabel(custom: CustomService, status: ServiceStatus | undefined): string {
    if (status?.state === "running" && status.port !== null) {
      return `127.0.0.1:${status.port}`;
    }
    return `127.0.0.1:${custom.hostPort}`;
  }
}
