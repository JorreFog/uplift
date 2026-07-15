import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import type { LibraryPayload, Settings } from "./types";

class AppState {
  library = $state<LibraryPayload>({ games: [], releases: [], changelogs: [], swaps: [] });
  settings = $state<Settings>({
    poll_hours: 6, minimize_to_tray: true,
    notify_on_new_release: true, launch_at_startup: false,
    theme: "dark", accent: "copper",
  });
  busy = $state(false);
  scanning = $state(false);
  toasts = $state<{ id: number; kind: "ok" | "err"; text: string }[]>([]);
  selectedGameId = $state<number | null>(null);

  #toastId = 0;

  toast(kind: "ok" | "err", text: string) {
    const id = ++this.#toastId;
    this.toasts.push({ id, kind, text });
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    }, kind === "err" ? 7000 : 3500);
  }

  async init() {
    try {
      this.library = await api.getLibrary();
      this.settings = await api.getSettings();
    } catch (e) {
      this.toast("err", String(e));
    }
    // Backend emits this after each background cycle.
    await listen("uplift://refreshed", async () => {
      try { this.library = await api.getLibrary(); } catch { /* window may be closing */ }
    });
    // First run: no games yet → scan automatically.
    if (this.library.games.length === 0) this.scan();
  }

  async scan() {
    this.scanning = true;
    try {
      this.library = await api.scanGames();
      this.toast("ok", `Scan complete — ${this.library.games.length} games with swappable DLLs`);
    } catch (e) {
      this.toast("err", String(e));
    } finally {
      this.scanning = false;
    }
  }

  /** Run an api call that returns a fresh LibraryPayload. */
  async mutate(fn: () => Promise<LibraryPayload>, okMsg?: string): Promise<boolean> {
    this.busy = true;
    try {
      this.library = await fn();
      if (okMsg) this.toast("ok", okMsg);
      return true;
    } catch (e) {
      this.toast("err", String(e));
      return false;
    } finally {
      this.busy = false;
    }
  }
}

export const app = new AppState();
