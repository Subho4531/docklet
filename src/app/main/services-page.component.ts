import { Component, computed, inject } from "@angular/core";

import { DockletStore } from "../docklet.store";
import type { ServiceStatus } from "../models";
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
