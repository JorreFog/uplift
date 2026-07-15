<script lang="ts">
  import { versionCmp, type Release } from "../types";

  let {
    releases,
    installed,
    onselect,
    selected,
  }: {
    releases: Release[]; // all releases of one family, any order
    installed: string; // currently installed version
    onselect: (version: string) => void;
    selected: string | null;
  } = $props();

  // Oldest → newest along the trace, capped to the most recent 8 pads.
  const sorted = $derived(
    [...releases].sort((a, b) => versionCmp(a.version, b.version)).slice(-8)
  );

  const installedKnown = $derived(sorted.some((r) => r.version === installed));

  function padState(r: Release): string {
    if (r.version === installed) return "installed";
    if (versionCmp(r.version, installed) > 0) return "ahead";
    return "past";
  }
</script>

<div class="rail" role="listbox" aria-label="Available versions">
  <!-- the copper trace -->
  <div class="trace" aria-hidden="true"></div>
  {#if !installedKnown && installed !== "unknown"}
    <div class="pad-wrap">
      <span class="pad installed off-manifest" title="Installed version (not in the release manifest)"></span>
      <span class="label mono current">{installed}</span>
    </div>
  {/if}
  {#each sorted as r (r.version)}
    <div class="pad-wrap">
      <button
        class="pad {padState(r)}"
        class:selected={selected === r.version}
        role="option"
        aria-selected={selected === r.version}
        title={r.release_date ?? r.version}
        onclick={() => onselect(r.version)}
      >
        {#if r.downloaded}<span class="dot" title="In your library"></span>{/if}
      </button>
      <span class="label mono" class:current={r.version === installed}>
        {r.version}
      </span>
    </div>
  {/each}
</div>

<style>
  .rail {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 26px;
    padding: 12px 6px 2px;
    overflow-x: auto;
  }
  .trace {
    position: absolute;
    left: 10px;
    right: 10px;
    top: 21px;
    height: 2px;
    background: linear-gradient(90deg, var(--copper-dim), var(--copper));
    opacity: 0.55;
  }
  .pad-wrap {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    min-width: 56px;
  }
  .pad {
    width: 18px;
    height: 18px;
    padding: 0;
    border-radius: 50%;
    background: var(--bg);
    border: 2px solid var(--copper-dim);
    display: grid;
    place-items: center;
    transition: border-color 120ms, box-shadow 120ms;
  }
  .pad:hover { border-color: var(--copper); }
  .pad.installed {
    border-color: var(--copper);
    box-shadow: 0 0 0 4px var(--copper-glow);
  }
  .pad.off-manifest { border-style: dashed; }
  .pad.past { opacity: 0.6; }
  .pad.selected {
    border-color: var(--copper);
    box-shadow: 0 0 0 3px var(--copper), 0 0 14px var(--copper-glow);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--mint);
  }
  .label {
    font-size: 11px;
    color: var(--faint);
    white-space: nowrap;
  }
  .label.current { color: var(--copper); }
</style>
