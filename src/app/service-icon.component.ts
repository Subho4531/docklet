import { Component, computed, input } from "@angular/core";

@Component({
  selector: "app-service-icon",
  template: `
    <img
      [src]="source()"
      alt=""
      aria-hidden="true"
      draggable="false"
      loading="eager"
    />
  `,
  styleUrl: "./service-icon.component.css",
})
export class ServiceIconComponent {
  readonly name = input.required<string>();
  readonly source = computed(() => `assets/services/${this.name()}.svg`);
}
