<script lang="ts">
  import { app } from "../stores.svelte";
  import { api } from "../api";
  import {
    FAMILY_LABEL,
    PLATFORM_LABEL,
    latestFor,
    presetsFor,
    recommendedPreset,
    versionCmp,
    type Family,
    type Game,
    type GamePresets,
  } from "../types";
  import VersionRail from "./VersionRail.svelte";

  let { game, onclose }: { game: Game; onclose: () => void } = $props();

  // family -> version chosen on the rail
  let picked = $state<Record<string, string>>({});
  let confirmingAc = $state<{ family: Family; version: string } | null>(null);

  // ---- driver preset overrides ----------------------------------------------
  let presets = $state<GamePresets | null>(null);
  let presetPick = $state<Record<string, number>>({});
  let applyingPreset = $state(false);

  function currentOverride(family: Family): number {
    if (!presets) return 0;
    return (family === "dlss" ? presets.sr : presets.rr) ?? 0;
  }

  $effect(() => {
    const id = game.id;
    presets = null;
    presetPick = {};
    api.getGamePresets(id).then((p) => {
      if (id !== game.id) return;
      presets = p;
      // Default choice: the active override, else the recommended preset.
      for (const family of ["dlss", "dlss_d"] as Family[]) {
        const current = currentOverride(family);
        presetPick[family] =
          current !== 0
            ? current
            : recommendedPreset(app.library.changelogs, family) ?? 0;
      }
    }).catch(() => { presets = { available: false, exe: null, sr: null, rr: null }; });
  });

  // Before/after proof (frametime capture) is temporarily disabled in the UI
  // while the capture path is being reworked; backend commands remain.

  async function applyPreset(family: Family) {
    applyingPreset = true;
    try {
      presets = await api.setGamePreset(game.id, family, presetPick[family] ?? 0);
      const opt = presetsFor(family)?.find((o) => o.value === presetPick[family]);
      app.toast("ok", presetPick[family] === 0
        ? `Cleared ${FAMILY_LABEL[family]} preset override`
        : `${FAMILY_LABEL[family]} preset set to ${opt?.label ?? presetPick[family]}`);
    } catch (e) {
      app.toast("err", String(e));
    } finally {
      applyingPreset = false;
    }
  }

  const acFlag = $derived(game.anticheat); // "warn:EAC" | "block:..." | null
  const acBlocked = $derived(acFlag?.startsWith("block") ?? false);

  function releasesOf(family: Family) {
    return app.library.releases.filter((r) => r.family === family);
  }

  /** Changelog entries between installed (exclusive) and target (inclusive). */
  function changelogDiff(family: Family, from: string, to: string) {
    return app.library.changelogs
      .filter(
        (c) =>
          c.family === family &&
          versionCmp(c.version, from) > 0 &&
          versionCmp(c.version, to) <= 0
      )
      .sort((a, b) => versionCmp(b.version, a.version));
  }

  async function doSwap(family: Family, version: string, confirmed = false) {
    const ok = await app.mutate(
      () => api.swapDll(game.id, family, version, confirmed),
      `Swapped ${FAMILY_LABEL[family]} to ${version}`
    );
    if (!ok) {
      // Backend signals "warn"-severity anti-cheat with a structured error.
      const last = app.toasts.at(-1);
      if (last?.text.startsWith("anticheat_confirm:")) {
        app.toasts = app.toasts.filter((t) => t.id !== last.id);
        confirmingAc = { family, version };
      }
    } else {
      confirmingAc = null;
      delete picked[family];
    }
  }

  async function toggleAuto() {
    const prefs = { ...game.prefs, auto_update: !game.prefs.auto_update };
    try {
      await api.setGamePrefs(game.id, prefs);
      game.prefs.auto_update = prefs.auto_update;
    } catch (e) {
      app.toast("err", String(e));
    }
  }

  async function toggleReapply() {
    const prefs = { ...game.prefs, reapply: !game.prefs.reapply };
    try {
      await api.setGamePrefs(game.id, prefs);
      game.prefs.reapply = prefs.reapply;
    } catch (e) {
      app.toast("err", String(e));
    }
  }

  async function togglePin(family: Family, version: string) {
    const pins = { ...game.prefs.pins };
    pins[family] = pins[family] === version ? "" : version;
    try {
      await api.setGamePrefs(game.id, { ...game.prefs, pins });
      game.prefs.pins = pins;
    } catch (e) {
      app.toast("err", String(e));
    }
  }
</script>

<div
  class="scrim"
  onclick={onclose}
  onkeydown={(e) => e.key === "Escape" && onclose()}
  role="button"
  tabindex="-1"
  aria-label="Close panel"
></div>

<aside aria-label={game.name}>
  <header>
    <div>
      <h2>{game.name}</h2>
      <p class="sub">
        {PLATFORM_LABEL[game.platform]} · <span class="mono path">{game.install_dir}</span>
      </p>
    </div>
    <button class="ghost" onclick={onclose} aria-label="Close">✕</button>
  </header>

  {#if acFlag}
    <div class="ac-banner" class:block={acBlocked}>
      {#if acBlocked}
        This game uses {acFlag.split(":")[1]}. Swapping DLLs risks a ban, so Uplift will not swap them here.
      {:else}
        This game uses {acFlag.split(":")[1]}. Manual swaps need confirmation; auto-update stays off.
      {/if}
    </div>
  {/if}

  <div class="auto-row">
    <label>
      <input
        type="checkbox"
        checked={game.prefs.auto_update}
        disabled={acFlag !== null}
        onchange={toggleAuto}
      />
      Auto-update DLLs in this game when new versions release
    </label>
    <label>
      <input
        type="checkbox"
        checked={game.prefs.reapply}
        disabled={acFlag !== null}
        onchange={toggleReapply}
      />
      Re-apply my chosen version when a game update reverts it
    </label>
  </div>

  {#each game.dlls as dll (dll.path)}
    {@const latest = latestFor(app.library.releases, dll.family)}
    {@const behind = latest && versionCmp(latest.version, dll.version) > 0}
    {@const pin = game.prefs.pins[dll.family] || ""}
    {@const target = picked[dll.family] ?? null}
    <section>
      <div class="fam-head">
        <h3>{FAMILY_LABEL[dll.family]}</h3>
        <span class="ver" class:latest={!behind} class:behind>{dll.version}</span>
        {#if behind}
          <span class="hint">→ {latest?.version} available</span>
        {/if}
        {#if pin}
          <span class="pin" title="Auto-update will not move this family">pinned {pin}</span>
        {/if}
      </div>
      <p class="dllpath mono">{dll.file_name}</p>

      {#if presets?.available && presetsFor(dll.family)}
        {@const presetOpts = presetsFor(dll.family)!}
        {@const current = currentOverride(dll.family)}
        {@const pick = presetPick[dll.family] ?? 0}
        {@const recommended = recommendedPreset(app.library.changelogs, dll.family)}
        <div class="preset-box">
          <div class="preset-head">
            <span>Driver preset</span>
            {#if current !== 0}
              <span class="override-tag">override active: {presetOpts.find((o) => o.value === current)?.label ?? current}</span>
            {/if}
          </div>
          <div class="preset-controls">
            <select bind:value={presetPick[dll.family]}>
              {#each presetOpts as opt}
                <option value={opt.value}>
                  {opt.label}{opt.value === recommended ? " (recommended)" : ""}
                </option>
              {/each}
            </select>
            {#if pick !== current}
              <button class="primary" disabled={applyingPreset} onclick={() => applyPreset(dll.family)}>
                {applyingPreset ? "Applying…" : "Apply"}
              </button>
            {/if}
            {#if current !== 0 && pick === current}
              <button
                class="ghost"
                disabled={applyingPreset}
                onclick={() => { presetPick[dll.family] = 0; applyPreset(dll.family); }}
              >
                Clear override
              </button>
            {/if}
          </div>
          <p class="preset-desc">
            {presetOpts.find((o) => o.value === pick)?.desc}
          </p>
          {#if presets.exe}
            <p class="preset-exe mono">applies to {presets.exe} via the NVIDIA driver profile</p>
          {/if}
        </div>
      {/if}

      {#if releasesOf(dll.family).length > 0}
        <VersionRail
          releases={releasesOf(dll.family)}
          installed={dll.version}
          selected={target}
          onselect={(v) => (picked[dll.family] = v)}
        />
      {:else}
        <p class="none">No releases in the manifest for this family yet.</p>
      {/if}

      {#if target && target !== dll.version}
        {@const upgrade = versionCmp(target, dll.version) > 0}
        {@const diff = upgrade ? changelogDiff(dll.family, dll.version, target) : []}
        <div class="plan">
          <div class="plan-head">
            <span class="ver">{dll.version}</span>
            <span class="arrow">→</span>
            <span class="ver latest">{target}</span>
            <span class="hint">{upgrade ? "upgrade" : "downgrade"}</span>
          </div>
          {#if diff.length > 0}
            <ul class="changes">
              {#each diff as entry}
                <li>
                  <span class="mono cver">{entry.version}</span>
                  {#each entry.changes as change}
                    <p>{change}</p>
                  {/each}
                  {#if entry.recommended_preset}
                    <p class="preset">Recommended preset: {entry.recommended_preset}</p>
                  {/if}
                  {#each entry.known_issues as issue}
                    <p class="issue">Known issue: {issue}</p>
                  {/each}
                </li>
              {/each}
            </ul>
          {/if}
          {#if confirmingAc && confirmingAc.family === dll.family}
            <div class="confirm">
              <p>
                {game.name} is flagged for {acFlag?.split(":")[1]}. Swap anyway?
              </p>
              <button class="danger" onclick={() => doSwap(dll.family, target, true)}>
                Swap anyway
              </button>
              <button class="ghost" onclick={() => (confirmingAc = null)}>Cancel</button>
            </div>
          {:else}
            <div class="actions">
              <button
                class="primary"
                disabled={app.busy || acBlocked}
                onclick={() => doSwap(dll.family, target)}
              >
                {app.busy ? "Working…" : `Swap to ${target}`}
              </button>
              <button onclick={() => togglePin(dll.family, target)}>
                {pin === target ? "Unpin" : "Pin this version"}
              </button>
              <button class="ghost" onclick={() => delete picked[dll.family]}>Cancel</button>
            </div>
          {/if}
        </div>
      {/if}

      {#if dll.has_backup}
        <button
          class="restore"
          disabled={app.busy}
          onclick={() =>
            app.mutate(
              () => api.restoreDll(game.id, dll.family),
              `Restored original ${FAMILY_LABEL[dll.family]} DLL`
            )}
        >
          Restore original DLL
        </button>
      {/if}
    </section>
  {/each}
</aside>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(6, 8, 11, 0.6);
    backdrop-filter: blur(2px);
    border: none;
    z-index: 10;
  }
  aside {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(560px, 92vw);
    background: var(--panel);
    border-left: 1px solid var(--line);
    padding: 24px 26px 40px;
    overflow-y: auto;
    z-index: 11;
    animation: slide-in 180ms ease-out;
  }
  @keyframes slide-in {
    from { transform: translateX(24px); opacity: 0; }
    to { transform: none; opacity: 1; }
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 14px;
  }
  h2 { font-size: 19px; }
  .sub { color: var(--muted); font-size: 12.5px; margin: 4px 0 0; }
  .path { font-size: 11px; color: var(--faint); word-break: break-all; }

  .ac-banner {
    border: 1px solid rgba(230, 200, 110, 0.4);
    color: var(--amber);
    background: rgba(230, 200, 110, 0.06);
    border-radius: 8px;
    padding: 10px 12px;
    font-size: 13px;
    margin-bottom: 14px;
  }
  .ac-banner.block {
    border-color: rgba(228, 116, 109, 0.5);
    color: var(--red);
    background: rgba(228, 116, 109, 0.06);
  }

  .auto-row { margin-bottom: 8px; }
  .auto-row label { display: flex; gap: 8px; align-items: center; font-size: 13px; color: var(--muted); }
  .auto-row input:checked + * { color: var(--ink); }

  section {
    border-top: 1px solid var(--line);
    padding: 18px 0 14px;
  }
  .fam-head { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  h3 { font-size: 14.5px; }
  .hint { color: var(--muted); font-size: 12px; }
  .pin {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--copper);
    border: 1px solid var(--copper-dim);
    border-radius: 6px;
    padding: 1px 7px;
  }
  .dllpath { font-size: 11px; color: var(--faint); margin: 4px 0 6px; }
  .none { color: var(--faint); font-size: 12.5px; }

  .preset-box {
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
    margin: 8px 0 10px;
  }
  .preset-head {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: 12.5px;
    color: var(--muted);
    margin-bottom: 6px;
  }
  .override-tag {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--mint);
    border: 1px solid rgba(111, 211, 166, 0.4);
    border-radius: 6px;
    padding: 1px 7px;
  }
  .preset-controls { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .preset-desc { color: var(--muted); font-size: 12.5px; margin: 6px 0 0; }
  .preset-exe { color: var(--faint); font-size: 11px; margin: 4px 0 0; }

  .plan {
    margin-top: 12px;
    background: var(--panel-2);
    border: 1px solid var(--line-bright);
    border-radius: var(--radius);
    padding: 14px;
  }
  .plan-head { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
  .arrow { color: var(--copper); }
  .changes { list-style: none; margin: 8px 0 12px; padding: 0; display: grid; gap: 10px; }
  .changes li { border-left: 2px solid var(--copper-dim); padding-left: 10px; }
  .changes p { margin: 2px 0; font-size: 13px; }
  .cver { color: var(--copper); font-size: 12px; }
  .preset { color: var(--mint); }
  .issue { color: var(--amber); }
  .actions, .confirm { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .confirm p { margin: 0; color: var(--amber); font-size: 13px; flex-basis: 100%; }
  .restore { margin-top: 10px; }
</style>
