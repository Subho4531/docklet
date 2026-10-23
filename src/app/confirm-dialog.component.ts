import { Component, HostListener, input, output } from "@angular/core";

@Component({
  selector: "app-confirm-dialog",
  template: `
    <div class="dialog-backdrop" (click)="cancelled.emit()">
      <section
        class="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-message"
        (click)="$event.stopPropagation()"
      >
        <span class="dialog-mark" [class.is-danger]="danger()" aria-hidden="true">
          <svg viewBox="0 0 20 20" fill="none">
            <path d="M10 3.4l6.7 11.8H3.3L10 3.4z" />
            <path d="M10 7.8v3.3M10 13.3v.1" />
          </svg>
        </span>
        <div class="dialog-copy">
          <h2 id="confirm-title">{{ title() }}</h2>
          <p id="confirm-message">{{ message() }}</p>
        </div>
        <div class="dialog-actions">
          <button type="button" class="quiet-btn" (click)="cancelled.emit()">
            Cancel
          </button>
          <button
            type="button"
            class="primary-btn"
            [class.is-danger]="danger()"
            (click)="confirmed.emit()"
          >
            {{ confirmLabel() }}
          </button>
        </div>
      </section>
    </div>
  `,
  styleUrl: "./confirm-dialog.component.css",
})
export class ConfirmDialogComponent {
  readonly title = input.required<string>();
  readonly message = input.required<string>();
  readonly confirmLabel = input("Confirm");
  readonly danger = input(true);
  readonly confirmed = output<void>();
  readonly cancelled = output<void>();

  @HostListener("document:keydown.escape")
  onEscape(): void {
    this.cancelled.emit();
  }
}
