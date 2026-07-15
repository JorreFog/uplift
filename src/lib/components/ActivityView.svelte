<script lang="ts">
  import { app } from "../stores.svelte";
  import { FAMILY_LABEL } from "../types";

  function gameName(id: number): string {
    return app.library.games.find((g) => g.id === id)?.name ?? "removed game";
  }
  function when(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }
</script>

<header>
  <h1>Activity</h1>
  <p class="sub">Every swap Uplift has made — manual and automatic</p>
</header>

{#if app.library.swaps.length === 0}
  <div class="empty"><p>No swaps yet. Pick a game in the library to get started.</p></div>
{:else}
  <div class="feed">
    {#each app.library.swaps as swap (swap.id)}
      <article>
        <div class="row">
          <strong>{gameName(swap.game_id)}</strong>
          <span class="fam">{FAMILY_LABEL[swap.family]}</span>
          {#if swap.automatic}<span class="auto">AUTO</span>{/if}
          <span class="time mono">{when(swap.at)}</span>
        </div>
        <div class="row versions">
          <span class="ver">{swap.from_version}</span>
          <span class="arrow">→</span>
          <span class="ver latest">{swap.to_version}</span>
        </div>
      </article>
    {/each}
  </div>
{/if}

<style>
  header { margin-bottom: 18px; }
  h1 { font-size: 22px; }
  .sub { color: var(--muted); margin: 4px 0 0; font-size: 13px; }
  .feed { display: grid; gap: 10px; max-width: 680px; }
  article {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px 14px;
  }
  .row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .fam { color: var(--muted); font-size: 12.5px; }
  .auto {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--mint);
    border: 1px solid rgba(111, 211, 166, 0.4);
    border-radius: 5px;
    padding: 0 6px;
  }
  .time { margin-left: auto; color: var(--faint); font-size: 11.5px; }
  .versions { margin-top: 6px; }
  .arrow { color: var(--copper); }
  .empty { color: var(--muted); padding: 60px 0; text-align: center; }
</style>
