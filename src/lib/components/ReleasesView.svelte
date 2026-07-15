<script lang="ts">
  import { app } from "../stores.svelte";
  import { api } from "../api";
  import { FAMILY_LABEL, versionCmp, type Family } from "../types";

  let family = $state<Family>("dlss");
  const families: Family[] = ["dlss", "dlss_g", "dlss_d", "xess", "xess_fg", "fsr"];

  const rows = $derived(
    app.library.releases
      .filter((r) => r.family === family)
      .sort((a, b) => versionCmp(b.version, a.version))
      .map((r) => ({
        release: r,
        log: app.library.changelogs.find(
          (c) => c.family === family && c.version === r.version
        ),
      }))
  );

  let refreshing = $state(false);
  async function refresh() {
    refreshing = true;
    try {
      const summary = await api.refreshRemote();
      app.library = await api.getLibrary();
      app.toast("ok", `Checked for updates — ${summary}`);
    } catch (e) {
      app.toast("err", String(e));
    } finally {
      refreshing = false;
    }
  }
</script>

<header>
  <div>
    <h1>DLL releases</h1>
    <p class="sub">Every known build, with community-sourced change notes</p>
  </div>
  <button onclick={refresh} disabled={refreshing}>
    {refreshing ? "Checking…" : "Check for updates now"}
  </button>
</header>

<div class="tabs" role="tablist">
  {#each families as f}
    <button
      role="tab"
      aria-selected={family === f}
      class:active={family === f}
      onclick={() => (family = f)}
    >
      {FAMILY_LABEL[f]}
    </button>
  {/each}
</div>

{#if rows.length === 0}
  <div class="empty">
    <p>No releases in the manifest for {FAMILY_LABEL[family]} yet. Check for updates, or add entries to the manifest repo.</p>
  </div>
{:else}
  <div class="list">
    {#each rows as { release, log } (release.version)}
      <article>
        <div class="head">
          <span class="ver">{release.version}</span>
          {#if release.release_date}
            <span class="date mono">{release.release_date}</span>
          {/if}
          {#if release.downloaded}
            <span class="in-lib">in library</span>
          {:else}
            <button
              disabled={app.busy}
              onclick={() =>
                app.mutate(
                  () => api.downloadDll(release.family, release.version),
                  `Downloaded ${FAMILY_LABEL[release.family]} ${release.version}`
                )}
            >
              Download
            </button>
          {/if}
        </div>
        {#if log}
          {#each log.changes as change}
            <p>{change}</p>
          {/each}
          {#if log.recommended_preset}
            <p class="preset">Recommended preset: {log.recommended_preset}</p>
          {/if}
          {#each log.known_issues as issue}
            <p class="issue">Known issue: {issue}</p>
          {/each}
        {:else}
          <p class="nolog">No change notes for this build yet — contributions welcome.</p>
        {/if}
      </article>
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
    margin-bottom: 18px;
  }
  h1 { font-size: 22px; }
  .sub { color: var(--muted); margin: 4px 0 0; font-size: 13px; }
  .tabs { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 18px; }
  .tabs button { border-radius: 999px; padding: 6px 14px; }
  .tabs button.active { border-color: var(--copper); color: var(--copper); }
  .list { display: grid; gap: 12px; max-width: 760px; }
  article {
    background: var(--panel);
    border: 1px solid var(--line);
    border-left: 2px solid var(--copper-dim);
    border-radius: var(--radius);
    padding: 14px 16px;
  }
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 6px; }
  .date { color: var(--faint); font-size: 12px; }
  .in-lib { color: var(--mint); font-size: 12px; margin-left: auto; }
  .head button { margin-left: auto; }
  article p { margin: 3px 0; font-size: 13.5px; }
  .preset { color: var(--mint); }
  .issue { color: var(--amber); }
  .nolog { color: var(--faint); }
  .empty { color: var(--muted); padding: 60px 0; text-align: center; }
</style>
