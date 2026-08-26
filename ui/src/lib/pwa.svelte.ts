type InstallChoice = {
  outcome: "accepted" | "dismissed";
  platform: string;
};

type BeforeInstallPromptEvent = Event & {
  prompt: () => Promise<void>;
  userChoice: Promise<InstallChoice>;
};

type NavigatorWithStandalone = Navigator & {
  standalone?: boolean;
};

class PwaManager {
  online = $state(true);
  installed = $state(false);
  installAvailable = $state(false);
  updateAvailable = $state(false);
  installing = $state(false);
  checkingForUpdate = $state(false);
  installHint = $state(
    "Use your browser’s install command to add Pandan to this device.",
  );

  #initialized = false;
  #reloading = false;
  #installPrompt: BeforeInstallPromptEvent | null = null;
  #registration: ServiceWorkerRegistration | null = null;
  #displayMode: MediaQueryList | null = null;
  #updateTimer: ReturnType<typeof setInterval> | undefined;

  initialize() {
    if (this.#initialized) return () => undefined;
    this.#initialized = true;
    this.online = navigator.onLine;
    this.#displayMode = window.matchMedia("(display-mode: standalone)");
    this.#syncInstalledState();
    this.#syncInstallHint();

    window.addEventListener("online", this.#handleOnline);
    window.addEventListener("offline", this.#handleOffline);
    window.addEventListener("beforeinstallprompt", this.#handleInstallPrompt);
    window.addEventListener("appinstalled", this.#handleInstalled);
    document.addEventListener("visibilitychange", this.#handleVisibilityChange);
    this.#displayMode.addEventListener("change", this.#handleDisplayModeChange);

    if ("serviceWorker" in navigator) {
      navigator.serviceWorker.addEventListener(
        "controllerchange",
        this.#handleControllerChange,
      );
      void navigator.serviceWorker.ready.then((registration) => {
        if (!this.#initialized) return;
        this.#registration = registration;
        this.#observeRegistration(registration);
        void this.checkForUpdate();
      });
      this.#updateTimer = setInterval(
        () => void this.checkForUpdate(),
        60 * 60 * 1_000,
      );
    }

    return () => this.destroy();
  }

  destroy() {
    if (!this.#initialized) return;
    this.#initialized = false;
    window.removeEventListener("online", this.#handleOnline);
    window.removeEventListener("offline", this.#handleOffline);
    window.removeEventListener(
      "beforeinstallprompt",
      this.#handleInstallPrompt,
    );
    window.removeEventListener("appinstalled", this.#handleInstalled);
    document.removeEventListener(
      "visibilitychange",
      this.#handleVisibilityChange,
    );
    this.#displayMode?.removeEventListener(
      "change",
      this.#handleDisplayModeChange,
    );
    if ("serviceWorker" in navigator) {
      navigator.serviceWorker.removeEventListener(
        "controllerchange",
        this.#handleControllerChange,
      );
    }
    clearInterval(this.#updateTimer);
    this.#updateTimer = undefined;
    this.#displayMode = null;
    this.#registration = null;
  }

  async install() {
    if (!this.#installPrompt || this.installing) return false;
    this.installing = true;
    const prompt = this.#installPrompt;
    try {
      await prompt.prompt();
      const choice = await prompt.userChoice;
      if (choice.outcome === "accepted") {
        this.#installPrompt = null;
        this.installAvailable = false;
      }
      return choice.outcome === "accepted";
    } finally {
      this.installing = false;
    }
  }

  async checkForUpdate() {
    if (!this.online || !this.#registration || this.checkingForUpdate) return;
    this.checkingForUpdate = true;
    try {
      await this.#registration.update();
      if (this.#registration.waiting && navigator.serviceWorker.controller) {
        this.updateAvailable = true;
      }
    } catch {
      // A failed update check should not interrupt an otherwise usable session.
    } finally {
      this.checkingForUpdate = false;
    }
  }

  activateUpdate() {
    const waiting = this.#registration?.waiting;
    if (!waiting) return;
    this.#reloading = true;
    waiting.postMessage({ type: "SKIP_WAITING" });
  }

  #observeRegistration(registration: ServiceWorkerRegistration) {
    if (registration.waiting && navigator.serviceWorker.controller) {
      this.updateAvailable = true;
    }
    registration.addEventListener("updatefound", () => {
      const installing = registration.installing;
      if (!installing) return;
      installing.addEventListener("statechange", () => {
        if (
          installing.state === "installed" &&
          navigator.serviceWorker.controller
        ) {
          this.updateAvailable = true;
        }
      });
    });
  }

  #syncInstalledState() {
    this.installed = Boolean(
      this.#displayMode?.matches ||
      (navigator as NavigatorWithStandalone).standalone,
    );
    if (this.installed) this.installAvailable = false;
  }

  #syncInstallHint() {
    const isIos =
      /iPad|iPhone|iPod/.test(navigator.userAgent) ||
      (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
    if (isIos) {
      this.installHint =
        "In your browser’s Share menu, choose Add to Home Screen.";
      return;
    }
    if (
      /Firefox/.test(navigator.userAgent) &&
      !/Android|Mobile/.test(navigator.userAgent)
    ) {
      this.installHint =
        "This browser does not offer desktop installation. Open Pandan in Chrome, Edge, or Safari to install it.";
      return;
    }
    if (
      /Macintosh/.test(navigator.userAgent) &&
      /Safari/.test(navigator.userAgent) &&
      !/Chrome/.test(navigator.userAgent)
    ) {
      this.installHint = "In Safari, choose File → Add to Dock.";
      return;
    }
    this.installHint =
      "Use the install icon in the address bar or your browser’s Install app command.";
  }

  #handleOnline = () => {
    this.online = true;
    void this.checkForUpdate();
  };

  #handleOffline = () => {
    this.online = false;
  };

  #handleInstallPrompt = (event: Event) => {
    event.preventDefault();
    this.#installPrompt = event as BeforeInstallPromptEvent;
    this.installAvailable = !this.installed;
  };

  #handleInstalled = () => {
    this.#installPrompt = null;
    this.installAvailable = false;
    this.installed = true;
  };

  #handleVisibilityChange = () => {
    if (document.visibilityState === "visible") void this.checkForUpdate();
  };

  #handleDisplayModeChange = () => {
    this.#syncInstalledState();
  };

  #handleControllerChange = () => {
    if (this.#reloading) window.location.reload();
  };
}

export const pwa = new PwaManager();
