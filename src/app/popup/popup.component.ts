import {
  Component,
  DestroyRef,
  ElementRef,
  HostListener,
  afterNextRender,
  computed,
  inject,
  viewChild,
} from "@angular/core";

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
  private readonly destroyRef = inject(DestroyRef);
  private readonly shell = viewChild.required<ElementRef<HTMLElement>>("popupShell");

  readonly visibleServices = computed(() =>
    this.store.services().filter((service) => service.state !== "missing"),
  );
  readonly subtitle = computed(() => {
    if (this.store.loading() && !this.store.snapshot()) {
      return "Checking services";
    }
    if (!this.store.dockerAvailable()) {
      return "Docker Desktop offline";
    }
    if (this.visibleServices().length === 0) {
      return "No services set up";
    }
    if (this.store.startingCount() > 0) {
      return `${this.store.runningCount()} running · ${this.store.startingCount()} starting`;
    }
    return `${this.store.runningCount()} of ${this.visibleServices().length} running`;
  });

  private fitTimer: ReturnType<typeof setTimeout> | null = null;
  private lastHeight = 0;
  private resizeObserver: ResizeObserver | null = null;

  constructor() {
    afterNextRender(() => {
      const shell = this.shell().nativeElement;
      this.resizeObserver = new ResizeObserver(() => this.scheduleFit());
      this.resizeObserver.observe(shell);
      this.scheduleFit();
    });

    this.destroyRef.onDestroy(() => {
      this.resizeObserver?.disconnect();
      if (this.fitTimer !== null) {
        clearTimeout(this.fitTimer);
      }
    });
  }

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

  private scheduleFit(): void {
    if (this.fitTimer !== null) {
      clearTimeout(this.fitTimer);
    }
    this.fitTimer = setTimeout(() => {
      this.fitTimer = null;
      void this.fitWindow();
    }, 40);
  }

  private async fitWindow(): Promise<void> {
    const contentHeight = Math.ceil(
      this.shell().nativeElement.getBoundingClientRect().height,
    );
    const chromeHeight = Math.max(0, window.outerHeight - window.innerHeight);
    const height = Math.max(180, Math.min(700, contentHeight + chromeHeight));

    if (Math.abs(height - this.lastHeight) < 1) {
      return;
    }
    this.lastHeight = height;

    try {
      await this.store.resizePopup(height);
    } catch {
      // Browser previews don't expose the native window bridge.
    }
  }
}
