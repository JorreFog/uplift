<script lang="ts">
  import { app } from "../stores.svelte";
  import { latestFor, versionCmp } from "../types";

  let { view = $bindable() }: { view: string } = $props();

  // Games with at least one DLL behind the latest known release.
  const outdatedCount = $derived(
    app.library.games.filter((g) =>
      g.dlls.some((d) => {
        const latest = latestFor(app.library.releases, d.family);
        return latest && versionCmp(latest.version, d.version) > 0;
      })
    ).length
  );

  const items = [
    { id: "library", label: "Library" },
    { id: "releases", label: "DLL releases" },
    { id: "activity", label: "Activity" },
    { id: "settings", label: "Settings" },
  ];
</script>

<nav>
  <div class="brand">
    <svg viewBox="0 0 32 32" width="26" height="26" aria-hidden="true">
      <rect x="1" y="1" width="30" height="30" rx="7" fill="var(--panel-2)" stroke="var(--line-bright)" />
      <line x1="7" y1="12" x2="23" y2="12" stroke="var(--copper)" stroke-width="2.4" />
      <path d="M22 8.5 L27 12 L22 15.5 Z" fill="var(--copper)" />
      <line x1="9" y1="20" x2="25" y2="20" stroke="var(--copper-dim)" stroke-width="2.4" />
      <path d="M10 16.5 L5 20 L10 23.5 Z" fill="var(--copper-dim)" />
    </svg>
    <span>Uplift</span>
  </div>

  {#each items as item}
    <button
      class="nav-item"
      class:active={view === item.id}
      onclick={() => (view = item.id)}
    >
      {item.label}
      {#if item.id === "library" && outdatedCount > 0}
        <span class="badge">{outdatedCount}</span>
      {/if}
    </button>
  {/each}

  <div class="foot">
    <button class="ghost" onclick={() => app.scan()} disabled={app.scanning}>
      {app.scanning ? "Scanning…" : "Rescan games"}
    </button>
  </div>
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 20px 14px;
    background: var(--panel);
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 18px;
    letter-spacing: 0.03em;
    padding: 4px 8px 18px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 8px;
    padding: 9px 12px;
    color: var(--muted);
    font-size: 13.5px;
  }
  .nav-item:hover { color: var(--ink); background: var(--panel-2); }
  .nav-item.active {
    color: var(--ink);
    background: var(--panel-2);
    box-shadow: inset 2px 0 0 var(--copper);
  }
  .badge {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--amber);
    border: 1px solid rgba(230, 200, 110, 0.35);
    border-radius: 999px;
    padding: 0 7px;
  }
  .foot { margin-top: auto; }
  .foot button { width: 100%; }
</style>
