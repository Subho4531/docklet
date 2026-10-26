(() => {
  const toast = document.querySelector(".toast");
  const switches = Array.from(document.querySelectorAll(".service-switch"));
  const count = document.querySelector("#running-count");
  let toastTimer;

  function showToast(message) {
    if (!toast) return;
    window.clearTimeout(toastTimer);
    toast.textContent = message;
    toast.classList.add("is-visible");
    toastTimer = window.setTimeout(() => {
      toast.classList.remove("is-visible");
    }, 3200);
  }

  function updateRunningCount() {
    if (!count) return;
    const running = switches.filter((service) =>
      service.classList.contains("is-on"),
    ).length;
    count.textContent = `${running} of ${switches.length} running`;
  }

  for (const service of switches) {
    service.addEventListener("click", () => {
      const isOn = service.classList.toggle("is-on");
      const status = service.querySelector("[data-status]");
      const name = service.dataset.service || "Service";
      const port = service.dataset.port;

      service.setAttribute("aria-checked", String(isOn));

      if (status) {
        status.textContent = isOn && port ? `Port ${port}` : "Stopped";
      }

      updateRunningCount();
      showToast(`${name} ${isOn ? "started" : "stopped"}. Preview only.`);
    });
  }

  const closePreview = document.querySelector("[data-preview-close]");
  if (closePreview) {
    closePreview.addEventListener("click", () => {
      showToast("Preview only. In Docklet, this hides the panel and services keep running.");
    });
  }

  async function writeClipboard(text) {
    if (navigator.clipboard && window.isSecureContext) {
      try {
        await navigator.clipboard.writeText(text);
        return true;
      } catch {
        // Fall through to the selection-based fallback.
      }
    }

    const field = document.createElement("textarea");
    field.value = text;
    field.setAttribute("readonly", "");
    field.style.position = "fixed";
    field.style.opacity = "0";
    document.body.appendChild(field);
    field.select();

    let copied = false;
    try {
      copied = document.execCommand("copy");
    } catch {
      copied = false;
    }

    field.remove();
    return copied;
  }

  for (const button of document.querySelectorAll("[data-copy]")) {
    const original = button.textContent;
    button.addEventListener("click", async () => {
      const copied = await writeClipboard(button.dataset.copy || "");
      button.textContent = copied ? "Copied" : "Copy failed";
      showToast(
        copied
          ? "Endpoint copied. Use the API key shown in Docklet."
          : "Could not copy the endpoint.",
      );

      window.setTimeout(() => {
        button.textContent = original;
      }, 1600);
    });
  }

  const revealItems = Array.from(document.querySelectorAll("[data-reveal]"));
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!("IntersectionObserver" in window) || reduceMotion) {
    revealItems.forEach((item) => item.classList.add("is-revealed"));
  } else {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.classList.add("is-revealed");
          observer.unobserve(entry.target);
        }
      },
      { rootMargin: "0px 0px -12% 0px", threshold: 0.08 },
    );

    revealItems.forEach((item) => {
      item.classList.add("is-reveal-ready");
      observer.observe(item);
    });
  }

  updateRunningCount();
})();
