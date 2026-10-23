import { Component, HostListener, computed, inject } from "@angular/core";

import { DockletStore } from "../docklet.store";
import type { ServiceStatus } from "../models";
import { ServiceIconComponent } from "../service-icon.component";
import { ToastComponent } from "../toast.component";

@Component({
  selector: "app-popup",
  imports: [ServiceIconComponent, ToastComponent],
  templateUrl: "./popup.component.html",
  styleUrl: "./popup.component.css",
})
export class PopupComponent {
  readonly store = inject(DockletStore);

  readonly total = computed(() => this.store.services().length);
  readonly subtitle = computed(() => {
    if (this.store.loading() && !this.store.snapshot()) {
      return "Checking services";
    }
    if (!this.store.dockerAvailable()) {
      return "Docker Desktop offline";
    }
    if (this.store.startingCount() > 0) {
      return `${this.store.runningCount()} running · ${this.store.startingCount()} starting`;
    }
    return `${this.store.runningCount()} of ${this.total()} running`;
  });

  @HostListener("document:keydown.escape")
  onEscape(): void {
    this.store.hidePopup();
  }

  openSettings(): void {
    void this.store.openMain("settings");
  }

  openApp(): void {
    void this.store.openMain("services");
  }

  toggle(service: ServiceStatus): void {
    void this.store.toggle(service);
  }

  copy(event: Event, service: ServiceStatus): void {
    event.preventDefault();
    void this.store.copyService(service);
  }

  onTileKeydown(event: KeyboardEvent, service: ServiceStatus): void {
    if (
      event.key === "ContextMenu" ||
      (event.shiftKey && event.key === "F10") ||
      (event.ctrlKey && event.key.toLowerCase() === "c")
    ) {
      event.preventDefault();
      void this.store.copyService(service);
    }
  }

  isOn(service: ServiceStatus): boolean {
    return service.state === "running" || service.state === "starting";
  }

  stateLabel(service: ServiceStatus): string {
    const operation = this.store.operationFor(service.id);
    if (operation === "start") return "Starting";
    if (operation === "stop") return "Stopping";
    if (operation === "reset") return "Resetting";

    switch (service.state) {
      case "running":
        return `Port ${service.port ?? service.hostPort}`;
      case "starting":
        return "Starting";
      case "stopped":
        return "Stopped";
      case "missing":
        return "Set up";
      case "dockerUnavailable":
        return "Docker offline";
      case "unmanaged":
        return "Name conflict";
      case "error":
        return "Needs attention";
    }
  }

  tileTitle(service: ServiceStatus): string {
    return `${service.name} · left-click to ${this.isOn(service) ? "stop" : "start"} · right-click to copy URL`;
  }
}
