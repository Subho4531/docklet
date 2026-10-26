import { Component, inject, signal } from "@angular/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inject as injectAnalytics } from "@vercel/analytics";

import { DockletStore } from "./docklet.store";
import { MainWindowComponent } from "./main/main-window.component";
import { PopupComponent } from "./popup/popup.component";

@Component({
  selector: "app-root",
  imports: [PopupComponent, MainWindowComponent],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent {
  private readonly store = inject(DockletStore);
  readonly mainWindow = signal(false);

  constructor() {
    // Initialize Vercel Analytics
    if (typeof window !== "undefined") {
      injectAnalytics();
    }

    let role: "popup" | "main" = "popup";
    try {
      role = getCurrentWindow().label === "main" ? "main" : "popup";
    } catch {
      // Browser previews use the quick-panel surface.
    }

    this.mainWindow.set(role === "main");
    this.store.init(role);
  }
}
