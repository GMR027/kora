/* =========================================================
   Kora — Landing page interactions
   ========================================================= */

(() => {
  "use strict";

  const root = document.documentElement;
  const body = document.body;

  /* ---------- Tema claro / oscuro ---------- */

  const themeToggle = document.querySelector(".theme-toggle");
  const themeMeta = document.querySelector('meta[name="theme-color"]');

  function storedTheme() {
    try {
      return localStorage.getItem("kora-theme");
    } catch {
      return null;
    }
  }

  function storeTheme(theme) {
    try {
      localStorage.setItem("kora-theme", theme);
    } catch {
      /* almacenamiento no disponible */
    }
  }

  function applyTheme(theme) {
    root.dataset.theme = theme;
    if (themeMeta) themeMeta.content = theme === "dark" ? "#151311" : "#fbf9f5";
    if (themeToggle) {
      const next = theme === "dark" ? "claro" : "oscuro";
      themeToggle.setAttribute("aria-label", `Cambiar al tema ${next}`);
      themeToggle.title = `Cambiar al tema ${next}`;
    }
  }

  applyTheme(root.dataset.theme === "dark" ? "dark" : "light");

  if (themeToggle) {
    themeToggle.addEventListener("click", () => {
      const next = root.dataset.theme === "dark" ? "light" : "dark";
      applyTheme(next);
      storeTheme(next);
    });
  }

  const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
  if (typeof systemTheme.addEventListener === "function") {
    systemTheme.addEventListener("change", (event) => {
      if (!storedTheme()) applyTheme(event.matches ? "dark" : "light");
    });
  }

  /* ---------- Header y menú móvil ---------- */

  const header = document.querySelector(".site-header");
  const menuButton = document.querySelector(".menu-btn");
  const navLinks = document.querySelector(".nav-links");

  function updateHeader() {
    if (!header) return;
    header.classList.toggle("scrolled", window.scrollY > 8);
  }

  updateHeader();
  window.addEventListener("scroll", updateHeader, { passive: true });

  function closeMenu() {
    if (!body.classList.contains("nav-open")) return;
    body.classList.remove("nav-open");
    if (menuButton) {
      menuButton.setAttribute("aria-expanded", "false");
      menuButton.setAttribute("aria-label", "Abrir menú");
    }
  }

  function openMenu() {
    body.classList.add("nav-open");
    if (menuButton) {
      menuButton.setAttribute("aria-expanded", "true");
      menuButton.setAttribute("aria-label", "Cerrar menú");
    }
  }

  if (menuButton) {
    menuButton.addEventListener("click", () => {
      if (body.classList.contains("nav-open")) closeMenu();
      else openMenu();
    });
  }

  if (navLinks) {
    navLinks.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", closeMenu);
    });
  }

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeMenu();
  });

  document.addEventListener("click", (event) => {
    if (!body.classList.contains("nav-open")) return;
    if (header && header.contains(event.target)) return;
    closeMenu();
  });

  window.addEventListener("resize", () => {
    if (window.innerWidth > 900) closeMenu();
  });

  /* ---------- Enlace activo en la navegación ---------- */

  const sectionLinks = new Map();
  document.querySelectorAll('.nav-links a[href^="#"]').forEach((link) => {
    const id = link.getAttribute("href").slice(1);
    const section = document.getElementById(id);
    if (section) sectionLinks.set(section, link);
  });

  if ("IntersectionObserver" in window && sectionLinks.size) {
    const navObserver = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;
          sectionLinks.forEach((link) => link.classList.remove("active"));
          const link = sectionLinks.get(entry.target);
          if (link) link.classList.add("active");
        });
      },
      { rootMargin: "-42% 0px -52% 0px", threshold: 0 },
    );

    sectionLinks.forEach((_link, section) => navObserver.observe(section));
  }

  /* ---------- Aparición al hacer scroll ---------- */

  const revealElements = Array.from(document.querySelectorAll(".reveal"));
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  if (!revealElements.length) {
    /* nada que animar */
  } else if (!("IntersectionObserver" in window) || reduceMotion) {
    revealElements.forEach((element) => element.classList.add("is-visible"));
  } else {
    revealElements.forEach((element) => {
      const siblings = element.parentElement
        ? Array.from(element.parentElement.children).filter((child) =>
            child.classList.contains("reveal"),
          )
        : [];
      const index = Math.max(0, siblings.indexOf(element));
      element.style.transitionDelay = `${Math.min(index, 4) * 55}ms`;
    });

    const revealObserver = new IntersectionObserver(
      (entries, observer) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        });
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.1 },
    );

    revealElements.forEach((element) => revealObserver.observe(element));
  }

  /* ---------- Toast ---------- */

  const toast = document.getElementById("toast");
  let toastTimer = null;

  function showToast(message) {
    if (!toast) return;
    clearTimeout(toastTimer);
    toast.textContent = message;
    toast.hidden = false;
    requestAnimationFrame(() => {
      toast.classList.add("is-visible");
    });
    toastTimer = setTimeout(() => {
      toast.classList.remove("is-visible");
      setTimeout(() => {
        toast.hidden = true;
      }, 220);
    }, 4400);
  }

  /* ---------- Copiar comandos ---------- */

  async function copyText(text) {
    try {
      if (navigator.clipboard && window.isSecureContext) {
        await navigator.clipboard.writeText(text);
        return true;
      }
    } catch {
      /* usa el respaldo */
    }

    try {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.opacity = "0";
      area.style.pointerEvents = "none";
      document.body.appendChild(area);
      area.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(area);
      return ok;
    } catch {
      return false;
    }
  }

  document.querySelectorAll("[data-copy]").forEach((button) => {
    button.addEventListener("click", async () => {
      const text = button.getAttribute("data-copy");
      if (!text) return;
      const copied = await copyText(text);
      showToast(
        copied
          ? "Comando copiado al portapapeles."
          : "No se pudo copiar. Selecciona el comando manualmente.",
      );
    });
  });

  /* ---------- Descargas: disponibilidad y sistema ---------- */

  const ua = navigator.userAgent.toLowerCase();

  function detectPlatform() {
    if (ua.includes("android")) return "android";
    if (/fedora|red hat|rhel|centos/.test(ua)) return "fedora";
    if (/ubuntu|debian|linux mint|zorin/.test(ua)) return "debian";
    if (ua.includes("linux") || ua.includes("x11")) return "debian";
    return null;
  }

  const platform = detectPlatform();
  if (platform) {
    const recommended = document.querySelector(`.download-card[data-platform="${platform}"]`);
    if (recommended) recommended.classList.add("is-recommended");
  }

  const buildCommand = {
    deb: "pnpm build:linux",
    rpm: "pnpm tauri build --bundles rpm",
    apk: "pnpm android:build:release",
  };

  function markMissing(button) {
    const card = button.closest(".download-card");
    if (!card || card.classList.contains("is-missing")) return;

    card.classList.add("is-missing");
    button.title = "Este paquete todavía no está publicado en esta copia de la landing.";

    const label = Array.from(button.childNodes).find(
      (node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim(),
    );
    if (label) label.textContent = "En preparación ";

    const file = button.getAttribute("data-file") || "";
    const kind = file.endsWith(".deb") ? "deb" : file.endsWith(".rpm") ? "rpm" : "apk";

    button.addEventListener("click", (event) => {
      event.preventDefault();
      const command = buildCommand[kind];
      showToast(
        `Este paquete aún no está en la copia publicada. Genéralo con: ${command}`,
      );
    });
  }

  if (location.protocol !== "file:") {
    document.querySelectorAll(".btn-download[data-file]").forEach(async (button) => {
      const url = button.getAttribute("data-file");
      if (!url) return;
      try {
        const response = await fetch(url, { method: "HEAD", cache: "no-store" });
        if (response.status === 404 || response.status === 410) markMissing(button);
      } catch {
        /* si el servidor no responde, deja el enlace tal cual */
      }
    });
  }

  /* ---------- FAQ: una respuesta abierta a la vez ---------- */

  const faqItems = Array.from(document.querySelectorAll(".faq-list details"));
  faqItems.forEach((item) => {
    item.addEventListener("toggle", () => {
      if (!item.open) return;
      faqItems.forEach((other) => {
        if (other !== item) other.open = false;
      });
    });
  });
})();
