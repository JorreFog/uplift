<script lang="ts">
  import { app } from "../stores.svelte";
  import {
    coverUrls,
    latestFor,
    versionCmp,
    FAMILY_SHORT,
    PLATFORM_LABEL,
    type Game,
  } from "../types";

  let { game }: { game: Game } = $props();

  // Walk the candidate list on load errors; null once exhausted → placeholder.
  let imgIndex = $state(0);
  const url = $derived(coverUrls(game)[imgIndex] ?? null);

  // deterministic hue per game name for the placeholder gradient
  const hue = $derived(
    [...game.name].reduce((a, c) => (a * 31 + c.charCodeAt(0)) % 360, 7)
  );

  function chipState(family: string): "latest" | "behind" | "unknown" {
    const dll = game.dlls.find((d) => d.family === family);
    if (!dll) return "unknown";
    const latest = latestFor(app.library.releases, dll.family);
    if (!latest || dll.version === "unknown") return "unknown";
    return versionCmp(latest.version, dll.version) > 0 ? "behind" : "latest";
  }

  const families = $derived([...new Set(game.dlls.map((d) => d.family))]);
</script>

<button class="card" onclick={() => (app.selectedGameId = game.id)}>
  <div class="cover">
    {#if url}
      <img src={url} alt="" loading="lazy" onerror={() => (imgIndex += 1)} />
    {:else}
      <div
        class="placeholder"
        style="background:
          radial-gradient(120% 90% at 20% 0%, hsl({hue} 32% 24%) 0%, transparent 60%),
          radial-gradient(120% 90% at 90% 100%, hsl({(hue + 40) % 360} 28% 18%) 0%, var(--panel-2) 70%)"
      >
        <span>{game.name.slice(0, 2).toUpperCase()}</span>
      </div>
    {/if}
    {#if game.anticheat}
      <span class="ac" class:block={game.anticheat.startsWith("block")}>
        {game.anticheat.split(":")[1]}
      </span>
    {/if}
    {#if game.prefs.auto_update}
      <span class="auto" title="Auto-update on">AUTO</span>
    {/if}
  </div>
  <div class="meta">
    <span class="name" title={game.name}>{game.name}</span>
    <span class="platform">{PLATFORM_LABEL[game.platform]}</span>
    <div class="chips">
      {#each families as f}
        <span class="chip {chipState(f)}">{FAMILY_SHORT[f]}</span>
      {/each}
    </div>
  </div>
</button>

<style>
  .card {
    display: flex;
    flex-direction: column;
    padding: 0;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    text-align: left;
    transition: transform 140ms, border-color 140ms, box-shadow 140ms;
  }
  .card:hover {
    transform: translateY(-2px);
    border-color: var(--copper-dim);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35), 0 0 0 1px var(--copper-glow);
  }
  .cover {
    position: relative;
    aspect-ratio: 2 / 3;
    background: var(--panel-2);
  }
  .cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .placeholder {
    width: 100%; height: 100%;
    display: grid; place-items: center;
  }
  .placeholder span {
    font-family: var(--font-display);
    font-size: 42px;
    color: rgba(232, 234, 237, 0.28);
    letter-spacing: 0.06em;
  }
  .ac, .auto {
    position: absolute;
    top: 8px;
    font-family: var(--font-mono);
    font-size: 10px;
    padding: 2px 7px;
    border-radius: 6px;
    backdrop-filter: blur(6px);
  }
  .ac {
    right: 8px;
    color: var(--amber);
    background: rgba(20, 16, 8, 0.65);
    border: 1px solid rgba(230, 200, 110, 0.4);
  }
  .ac.block { color: var(--red); border-color: rgba(228, 116, 109, 0.5); }
  .auto {
    left: 8px;
    color: var(--mint);
    background: rgba(8, 20, 14, 0.65);
    border: 1px solid rgba(111, 211, 166, 0.4);
  }
  .meta { display: flex; flex-direction: column; gap: 4px; padding: 10px 12px 12px; }
  .name {
    font-weight: 600;
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .platform { color: var(--faint); font-size: 11.5px; }
  .chips { display: flex; gap: 5px; margin-top: 4px; flex-wrap: wrap; }
  .chip {
    font-family: var(--font-mono);
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--line-bright);
    color: var(--muted);
  }
  .chip.latest { color: var(--mint); border-color: rgba(111, 211, 166, 0.4); }
  .chip.behind { color: var(--amber); border-color: rgba(230, 200, 110, 0.4); }
</style>
