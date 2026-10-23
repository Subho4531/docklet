import { Component, inject, input } from "@angular/core";

import { DockletStore } from "./docklet.store";

@Component({
  selector: "app-toast",
  template: `
    @if (store.toast(); as message) {
      <div
        class="toast"
        [class.is-danger]="message.tone === 'danger'"
        [class.is-compact]="compact()"
        [attr.role]="message.tone === 'danger' ? 'alert' : 'status'"
        aria-live="polite"
      >
        @if (message.tone === "danger") {
          <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <path d="M10 3.2l7 12.3H3L10 3.2z" />
            <path d="M10 8v3.4M10 13.7v.1" />
          </svg>
        } @else {
          <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <path d="M4 10.5l3.6 3.4L16 5.8" />
          </svg>
        }
        <span>{{ message.message }}</span>
      </div>
    }
  `,
  styleUrl: "./toast.component.css",
  host: { "[class.is-compact]": "compact()" },
})
export class ToastComponent {
  readonly store = inject(DockletStore);
  readonly compact = input(false);
}
