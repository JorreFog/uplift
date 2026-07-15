import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { app } from "./stores.svelte";

/** In-app self-update against GitHub releases (latest.json updater feed). */
class UpdaterState {
  current = $state("");
  available = $state<Update | null>(null);
  checking = $state(false);
  installing = $state(false);

  async init() {
    try {
      this.current = await getVersion();
    } catch {
      /* not running under tauri */
    }
    // Quiet check on startup; surfaces a toast only when there is an update.
    await this.check(true);
  }

  async check(quiet = false) {
    this.checking = true;
    try {
      const update = await check();
      this.available = update;
      if (update) {
        app.toast("ok", `Uplift ${update.version} is available — install it from Settings`);
      } else if (!quiet) {
        app.toast("ok", "Uplift is up to date");
      }
    } catch (e) {
      if (!quiet) app.toast("err", `Update check failed: ${e}`);
    } finally {
      this.checking = false;
    }
  }

  async install() {
    if (!this.available) return;
    this.installing = true;
    try {
      await this.available.downloadAndInstall();
      await relaunch();
    } catch (e) {
      app.toast("err", `Update failed: ${e}`);
      this.installing = false;
    }
  }
}

export const updater = new UpdaterState();
