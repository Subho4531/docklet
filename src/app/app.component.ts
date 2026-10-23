import { Component, inject, signal } from "@angular/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

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
