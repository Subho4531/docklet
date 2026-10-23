import { Component, computed, inject, signal } from "@angular/core";
import { FormsModule } from "@angular/forms";

import { DockletStore } from "../docklet.store";
import type { CustomService, DockerSearchResult, EnvVar } from "../models";
import { CustomServiceDialogComponent } from "./custom-service-dialog.component";

interface CatalogService {
  name: string;
  image: string;
  tagline: string;
  category: string;
  containerPort: number;
  hostPort: number;
  urlTemplate: string;
  volumePath: string | null;
  env: EnvVar[];
  healthCommand: string | null;
  monogram: string;
  accent: string;
}

const CATALOG: CatalogService[] = [
  {
    name: "Nginx",
    image: "nginx:1.27-alpine",
    tagline: "Static sites, reverse proxies, and lightweight web serving.",
    category: "Web",
    containerPort: 80,
    hostPort: 8080,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: null,
    env: [],
    healthCommand: "wget -qO- http://127.0.0.1:80",
    monogram: "NG",
    accent: "#6fbf73",
  },
  {
    name: "Adminer",
    image: "adminer:4.8.1",
    tagline: "A single-file database browser for SQL and NoSQL engines.",
    category: "Database UI",
    containerPort: 8080,
    hostPort: 8081,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: null,
    env: [],
    healthCommand: null,
    monogram: "AD",
    accent: "#6f9de0",
  },
  {
    name: "MariaDB",
    image: "mariadb:11.4",
    tagline: "Community MySQL-compatible database with a clean data volume.",
    category: "Database",
    containerPort: 3306,
    hostPort: 3307,
    urlTemplate: "mysql://docklet:docklet@127.0.0.1:{port}/docklet",
    volumePath: "/var/lib/mysql",
    env: [
      { key: "MARIADB_ROOT_PASSWORD", value: "docklet" },
      { key: "MARIADB_DATABASE", value: "docklet" },
      { key: "MARIADB_USER", value: "docklet" },
      { key: "MARIADB_PASSWORD", value: "docklet" },
    ],
    healthCommand: "/usr/local/bin/healthcheck.sh --connect --innodb_initialized",
    monogram: "MA",
    accent: "#4fb8b0",
  },
  {
    name: "RabbitMQ",
    image: "rabbitmq:4-management-alpine",
    tagline: "Message broker with its management console on the local port.",
    category: "Queue",
    containerPort: 15672,
    hostPort: 15672,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: "/var/lib/rabbitmq",
    env: [
      { key: "RABBITMQ_DEFAULT_USER", value: "docklet" },
      { key: "RABBITMQ_DEFAULT_PASS", value: "docklet" },
    ],
    healthCommand: "rabbitmq-diagnostics -q ping",
    monogram: "RQ",
    accent: "#e58b52",
  },
  {
    name: "Memcached",
    image: "memcached:1.6-alpine",
    tagline: "Tiny in-memory cache for fast ephemeral key-value workloads.",
    category: "Cache",
    containerPort: 11211,
    hostPort: 11211,
    urlTemplate: "memcached://127.0.0.1:{port}",
    volumePath: null,
    env: [],
    healthCommand: null,
    monogram: "MC",
    accent: "#7d8fe0",
  },
  {
    name: "Grafana",
    image: "grafana/grafana:11.4.0",
    tagline: "Dashboards and observability for metrics, logs, and traces.",
    category: "Monitoring",
    containerPort: 3000,
    hostPort: 3001,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: "/var/lib/grafana",
    env: [
      { key: "GF_SECURITY_ADMIN_USER", value: "admin" },
      { key: "GF_SECURITY_ADMIN_PASSWORD", value: "docklet" },
    ],
    healthCommand: null,
    monogram: "GF",
    accent: "#e6a84e",
  },
  {
    name: "Jaeger",
    image: "jaegertracing/all-in-one:1.62",
    tagline: "All-in-one distributed tracing UI and collector for development.",
    category: "Tracing",
    containerPort: 16686,
    hostPort: 16686,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: null,
    env: [],
    healthCommand: null,
    monogram: "JG",
    accent: "#e8bd67",
  },
  {
    name: "Uptime Kuma",
    image: "louislam/uptime-kuma:1",
    tagline: "Self-hosted uptime monitoring with a polished status dashboard.",
    category: "Monitoring",
    containerPort: 3001,
    hostPort: 3002,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: "/app/data",
    env: [],
    healthCommand: null,
    monogram: "UK",
    accent: "#63b97a",
  },
  {
    name: "Meilisearch",
    image: "getmeili/meilisearch:v1.11",
    tagline: "Fast typo-tolerant search engine with a developer-friendly API.",
    category: "Search",
    containerPort: 7700,
    hostPort: 7700,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: "/meili_data",
    env: [
      { key: "MEILI_ENV", value: "development" },
      { key: "MEILI_NO_ANALYTICS", value: "true" },
    ],
    healthCommand: null,
    monogram: "MS",
    accent: "#d9ae45",
  },
  {
    name: "n8n",
    image: "n8nio/n8n:1.70",
    tagline: "Visual workflow automation with hundreds of integrations.",
    category: "Automation",
    containerPort: 5678,
    hostPort: 5678,
    urlTemplate: "http://127.0.0.1:{port}",
    volumePath: "/home/node/.n8n",
    env: [
      { key: "N8N_SECURE_COOKIE", value: "false" },
      { key: "N8N_HOST", value: "127.0.0.1" },
    ],
    healthCommand: null,
    monogram: "N8",
    accent: "#dc6b70",
  },
];

@Component({
  selector: "app-library-page",
  imports: [FormsModule, CustomServiceDialogComponent],
  templateUrl: "./library-page.component.html",
  styleUrl: "./library-page.component.css",
})
export class LibraryPageComponent {
  readonly store = inject(DockletStore);

  readonly catalog = CATALOG;
  readonly query = signal("");
  readonly searchResults = signal<DockerSearchResult[]>([]);
  readonly searchBusy = signal(false);
  readonly hasSearched = signal(false);
  readonly searchError = signal<string | null>(null);
  readonly dialogOpen = signal(false);
  readonly pendingInstall = signal<CustomService | null>(null);
  readonly installing = signal(false);

  readonly existingIds = computed(() => {
    const ids = new Set(this.store.services().map((service) => service.id));
    for (const service of this.store.settings()?.customServices ?? []) {
      ids.add(service.id);
    }
    return [...ids];
  });

  readonly dialogTitle = computed(() => {
    const service = this.pendingInstall();
    return service ? `Install ${service.name}` : "Install service";
  });

  async search(): Promise<void> {
    const query = this.query().trim();
    if (query.length < 2) {
      this.searchResults.set([]);
      this.searchError.set(
        query.length === 0
          ? null
          : "Enter at least 2 characters to search Docker Hub.",
      );
      this.hasSearched.set(query.length > 0);
      return;
    }
    if (this.searchBusy()) {
      return;
    }

    this.query.set(query);
    this.searchBusy.set(true);
    this.hasSearched.set(true);
    this.searchError.set(null);
    try {
      const results = await this.store.searchDockerImages(query);
      this.searchResults.set(
        [...results].sort(
          (left, right) =>
            Number(right.isOfficial) - Number(left.isOfficial) ||
            right.starCount - left.starCount,
        ),
      );
    } catch (error) {
      this.searchResults.set([]);
      this.searchError.set(describeError(error));
    } finally {
      this.searchBusy.set(false);
    }
  }

  showCatalog(): void {
    this.query.set("");
    this.searchResults.set([]);
    this.hasSearched.set(false);
    this.searchError.set(null);
  }

  openCatalogService(item: CatalogService): void {
    this.pendingInstall.set({
      id: "",
      name: item.name,
      image: item.image,
      containerPort: item.containerPort,
      hostPort: this.suggestPort(item.hostPort),
      urlTemplate: item.urlTemplate,
      volumePath: item.volumePath,
      env: item.env.map((entry) => ({ ...entry })),
      healthCommand: item.healthCommand,
    });
    this.dialogOpen.set(true);
  }

  openSearchResult(result: DockerSearchResult): void {
    const name = displayName(result.name);
    this.pendingInstall.set({
      id: "",
      name,
      image: `${result.name}:latest`,
      containerPort: 80,
      hostPort: this.suggestPort(8080),
      urlTemplate: "http://127.0.0.1:{port}",
      volumePath: null,
      env: [],
      healthCommand: null,
    });
    this.dialogOpen.set(true);
  }

  closeDialog(): void {
    if (this.installing()) {
      return;
    }
    this.dialogOpen.set(false);
    this.pendingInstall.set(null);
  }

  async install(service: CustomService): Promise<void> {
    if (this.installing()) {
      return;
    }

    this.installing.set(true);
    try {
      const saved = await this.store.addCustomService(service);
      if (!saved) {
        return;
      }
      this.dialogOpen.set(false);
      this.pendingInstall.set(null);
      this.store.setMainPage("services");
      void this.store.startServiceById(service.id);
    } finally {
      this.installing.set(false);
    }
  }

  formatStars(count: number): string {
    return new Intl.NumberFormat("en", { notation: "compact" }).format(count);
  }

  private suggestPort(preferred: number): number {
    const used = new Set<number>();
    const settings = this.store.settings();
    for (const port of Object.values(settings?.servicePorts ?? {})) {
      used.add(port);
    }
    for (const service of settings?.customServices ?? []) {
      used.add(service.hostPort);
    }
    for (const service of this.store.services()) {
      used.add(service.port ?? service.hostPort);
    }

    let port = preferred;
    while (used.has(port) && port < 65535) {
      port += 1;
    }
    return port;
  }
}

function displayName(image: string): string {
  const parts = image.split("/");
  const repository = parts.length > 1 ? parts[0] : null;
  const base = parts[parts.length - 1]
    .replace(/[-_.]+/g, " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
  const label = repository ? `${base} (${repository})` : base;
  return label.slice(0, 48);
}

function describeError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Docker Hub search failed. Check your connection and try again.";
}
