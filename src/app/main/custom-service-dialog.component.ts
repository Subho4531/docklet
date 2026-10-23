import {
  Component,
  HostListener,
  OnInit,
  inject,
  input,
  output,
  signal,
} from "@angular/core";
import { FormsModule } from "@angular/forms";

import type { CustomService, EnvVar } from "../models";

interface CustomFormState {
  name: string;
  image: string;
  hostPort: number | null;
  containerPort: number | null;
  urlTemplate: string;
  volumePath: string;
  healthCommand: string;
  envText: string;
}

@Component({
  selector: "app-custom-service-dialog",
  imports: [FormsModule],
  templateUrl: "./custom-service-dialog.component.html",
  styleUrl: "./custom-service-dialog.component.css",
})
export class CustomServiceDialogComponent implements OnInit {
  readonly service = input<CustomService | null>(null);
  readonly existingIds = input<string[]>([]);
  readonly saved = output<CustomService>();
  readonly cancelled = output<void>();

  readonly form = signal<CustomFormState>({
    name: "",
    image: "",
    hostPort: 8080,
    containerPort: 80,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: "",
    healthCommand: "",
    envText: "",
  });
  readonly error = signal<string | null>(null);
  readonly editing = signal(false);

  ngOnInit(): void {
    const service = this.service();
    if (!service) {
      return;
    }

    this.editing.set(true);
    this.form.set({
      name: service.name,
      image: service.image,
      hostPort: service.hostPort,
      containerPort: service.containerPort,
      urlTemplate: service.urlTemplate,
      volumePath: service.volumePath ?? "",
      healthCommand: service.healthCommand ?? "",
      envText: service.env
        .map((item) => `${item.key}=${item.value}`)
        .join("\n"),
    });
  }

  @HostListener("document:keydown.escape")
  onEscape(): void {
    this.cancelled.emit();
  }

  patch<K extends keyof CustomFormState>(
    key: K,
    value: CustomFormState[K],
  ): void {
    this.form.update((current) => ({ ...current, [key]: value }));
    this.error.set(null);
  }

  save(): void {
    const form = this.form();
    const name = form.name.trim();
    const image = form.image.trim();
    const hostPort = Number(form.hostPort);
    const containerPort = Number(form.containerPort);
    const urlTemplate = form.urlTemplate.trim();

    if (!name) {
      this.error.set("Give the service a name.");
      return;
    }
    if (!image) {
      this.error.set("Enter a Docker image reference.");
      return;
    }
    if (!isValidPort(hostPort) || !isValidPort(containerPort)) {
      this.error.set("Ports must be between 1 and 65535.");
      return;
    }
    if (!urlTemplate.includes("{port}")) {
      this.error.set("The URL template must include {port}.");
      return;
    }

    const parsedEnv = parseEnvironment(form.envText);
    if (typeof parsedEnv === "string") {
      this.error.set(parsedEnv);
      return;
    }

    const original = this.service();
    const id = original?.id ?? this.uniqueId(slugify(name));

    this.saved.emit({
      id,
      name,
      image,
      hostPort,
      containerPort,
      urlTemplate,
      volumePath: form.volumePath.trim() || null,
      env: parsedEnv,
      healthCommand: form.healthCommand.trim() || null,
    });
  }

  private uniqueId(base: string): string {
    const normalized = base.length < 2 ? `${base || "service"}1` : base;
    const existing = new Set(this.existingIds());
    if (!existing.has(normalized)) {
      return normalized;
    }
    for (let index = 2; index < 100; index += 1) {
      const candidate = `${normalized}-${index}`.slice(0, 32);
      if (!existing.has(candidate)) {
        return candidate;
      }
    }
    return `${normalized.slice(0, 28)}-new`;
  }
}

function isValidPort(value: number): boolean {
  return Number.isInteger(value) && value > 0 && value <= 65535;
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 32);
}

function parseEnvironment(value: string): EnvVar[] | string {
  const env: EnvVar[] = [];
  const keys = new Set<string>();

  for (const rawLine of value.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }
    const separator = line.indexOf("=");
    if (separator <= 0) {
      return `Environment line \`${line}\` must use KEY=value.`;
    }
    const key = line.slice(0, separator).trim();
    const envValue = line.slice(separator + 1).trim();
    if (!/^[A-Z_][A-Z0-9_]*$/.test(key)) {
      return `Environment key \`${key}\` must use uppercase letters, numbers, and underscores.`;
    }
    if (keys.has(key)) {
      return `Environment key \`${key}\` is repeated.`;
    }
    keys.add(key);
    env.push({ key, value: envValue });
  }

  return env;
}
