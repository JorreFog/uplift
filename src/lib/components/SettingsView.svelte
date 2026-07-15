<script lang="ts">
  import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { onMount } from "svelte";
  import { app } from "../stores.svelte";
  import { updater } from "../updater.svelte";
  import { api } from "../api";

  let saving = $state(false);

  onMount(async () => {
    try {
      app.settings.launch_at_startup = await isEnabled();
    } catch {
      /* autostart plugin unavailable in dev on some setups */
    }
  });

  async function save() {
    saving = true;
    try {
      await api.setSettings($state.snapshot(app.settings));
      try {
        if (app.settings.launch_at_startup) await enable();
        else await disable();
      } catch {
        /* non-fatal */
      }
      app.toast("ok", "Settings saved");
    } catch (e) {
      app.toast("err", String(e));
    } finally {
      saving = false;
    }
  }
</script>

<header>
  <h1>Settings</h1>
  <p class="sub">How Uplift watches for releases and behaves in the background</p>
</header>

<div class="panel">
  <label class="row">
    <span>
      Check for new DLL releases every
      <select bind:value={app.settings.poll_hours}>
        {#each [1, 3, 6, 12, 24] as h}
          <option value={h}>{h} hour{h > 1 ? "s" : ""}</option>
        {/each}
      </select>
    </span>
  </label>

  <label class="row">
    <input type="checkbox" bind:checked={app.settings.notify_on_new_release} />
    <span>Show a notification when a new DLL version releases</span>
  </label>

  <label class="row">
    <input type="checkbox" bind:checked={app.settings.minimize_to_tray} />
    <span>Keep running in the tray when the window is closed<br />
      <small>Required for background release checks and auto-update.</small>
    </span>
  </label>

  <label class="row">
    <input type="checkbox" bind:checked={app.settings.launch_at_startup} />
    <span>Launch at startup (hidden in the tray)</span>
  </label>

  <button class="primary" onclick={save} disabled={saving}>
    {saving ? "Saving…" : "Save changes"}
  </button>
</div>

<div class="panel">
  <h3>App updates</h3>
  <p class="version">
    Uplift {updater.current || "(dev)"}
    {#if updater.available}
      <span class="badge">→ {updater.available.version} available</span>
    {/if}
  </p>
  {#if updater.available}
    <button class="primary" onclick={() => updater.install()} disabled={updater.installing}>
      {updater.installing ? "Installing… the app will restart" : `Install ${updater.available.version} and restart`}
    </button>
  {:else}
    <button onclick={() => updater.check()} disabled={updater.checking}>
      {updater.checking ? "Checking…" : "Check for app updates"}
    </button>
  {/if}
</div>

<div class="panel about">
  <h3>About auto-update</h3>
  <p>
    Auto-update is opt-in per game, never swaps a pinned version, skips any game
    on the anti-cheat list, and never touches a game while it is running. The
    original DLL is always backed up next to the swapped one and can be
    restored from the game's panel.
  </p>
</div>

<style>
  header { margin-bottom: 18px; }
  h1 { font-size: 22px; }
  .sub { color: var(--muted); margin: 4px 0 0; font-size: 13px; }
  .panel {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 18px 20px;
    max-width: 620px;
    display: grid;
    gap: 16px;
    margin-bottom: 16px;
  }
  .row { display: flex; gap: 10px; align-items: flex-start; font-size: 13.5px; }
  small { color: var(--faint); }
  select { margin: 0 4px; }
  button { justify-self: start; }
  .about h3 { font-size: 14px; margin-bottom: 6px; }
  .about p { color: var(--muted); font-size: 13px; margin: 0; }
  h3 { font-size: 14px; }
  .version { color: var(--muted); font-size: 13px; margin: 0; font-family: var(--font-mono); }
  .badge { color: var(--mint); margin-left: 6px; }
</style>
