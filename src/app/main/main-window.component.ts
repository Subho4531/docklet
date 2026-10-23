import { Component, inject } from "@angular/core";

import { DockletStore } from "../docklet.store";
import type { MainPage } from "../models";
import { ToastComponent } from "../toast.component";
import { ServicesPageComponent } from "./services-page.component";
import { SettingsPageComponent } from "./settings-page.component";

@Component({
  selector: "app-main-window",
  imports: [ServicesPageComponent, SettingsPageComponent, ToastComponent],
  templateUrl: "./main-window.component.html",
  styleUrl: "./main-window.component.css",
})
export class MainWindowComponent {
  readonly store = inject(DockletStore);

  navigate(page: MainPage): void {
    this.store.setMainPage(page);
  }
}
