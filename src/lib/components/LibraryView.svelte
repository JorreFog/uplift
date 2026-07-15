<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { app } from "../stores.svelte";
  import { api } from "../api";
  import { latestFor, versionCmp, PLATFORM_LABEL, type Game } from "../types";
  import GameCard from "./GameCard.svelte";

  let query = $state("");
  let onlyOutdated = $state(false);
  let platformFilter = $state("all");

  function isOutdated(g: Game): boolean {
    return g.dlls.some((d) => {
      const latest = latestFor(app.library.releases, d.family);
      return latest && versionCmp(latest.version, d.version) > 0;
    });
  }

  const platforms = $derived([
    ...new Set(app.library.games.map((g) => g.platform)),
  ]);

  const filtered = $derived(
    app.library.games.filter(
      (g) =>
        (!query || g.name.toLowerCase().includes(query.toLowerCase())) &&
        (platformFilter === "all" || g.platform === platformFilter) &&
        (!onlyOutdated || isOutdated(g))
    )
  );

  async function addManual() {
    const dir = await open({ directory: true, title: "Choose a game folder" });
    if (typeof dir !== "string") return;
    const name = dir.split(/[\\/]/).filter(Boolean).pop() ?? "Manual game";
    await app.mutate(() => api.addManualGame(name, dir), `Added ${name}`);
  }
</script>

<header>
  <div>
    <h1>Library</h1>
    <p class="sub">
      {app.library.games.length} games with swappable DLLs
    </p>
  </div>
  <div class="controls">
    <input type="text" placeholder="Search games" bind:value={query} />
    <select bind:value={platformFilter}>
      <option value="all">All platforms</option>
      {#each platforms as p}
        <option value={p}>{PLATFORM_LABEL[p]}</option>
      {/each}
    </select>
    <button
      class:primary={onlyOutdated}
      onclick={() => (onlyOutdated = !onlyOutdated)}
    >
      Updates available
    </button>
    <button onclick={addManual}>Add game</button>
  </div>
</header>

{#if app.scanning && app.library.games.length === 0}
  <div class="empty">
    <p>Scanning your launchers for games…</p>
  </div>
{:else if filtered.length === 0}
  <div class="empty">
    <p>
      {app.library.games.length === 0
        ? "No games with DLSS, FSR or XeSS DLLs were found. Install a supported game, or add one manually."
        : "Nothing matches the current filters."}
    </p>
    {#if app.library.games.length === 0}
      <button class="primary" onclick={() => app.scan()}>Scan again</button>
    {/if}
  </div>
{:else}
  <div class="grid">
    {#each filtered as game (game.id)}
      <GameCard {game} />
    {/each}
  </div>
{/if}

<style>
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 22px;
  }
  h1 { font-size: 22px; }
  .sub { color: var(--muted); margin: 4px 0 0; font-size: 13px; }
  .controls { display: flex; gap: 8px; flex-wrap: wrap; }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 16px;
  }
  .empty {
    display: grid;
    place-items: center;
    gap: 14px;
    padding: 80px 20px;
    color: var(--muted);
    text-align: center;
  }
</style>
