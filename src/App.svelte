<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { app } from "./lib/stores.svelte";
  import { updater } from "./lib/updater.svelte";
  import Splash from "./lib/components/Splash.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import LibraryView from "./lib/components/LibraryView.svelte";
  import ReleasesView from "./lib/components/ReleasesView.svelte";
  import ActivityView from "./lib/components/ActivityView.svelte";
  import SettingsView from "./lib/components/SettingsView.svelte";
  import GameDetail from "./lib/components/GameDetail.svelte";
  import Toasts from "./lib/components/Toasts.svelte";

  let view = $state<"library" | "releases" | "activity" | "settings">("library");
  let splash = $state(true);

  const selectedGame = $derived(
    app.library.games.find((g) => g.id === app.selectedGameId) ?? null
  );

  onMount(() => {
    app.init();
    updater.init();
  });

  // Theme and accent live on <html> so every CSS token follows instantly.
  $effect(() => {
    document.documentElement.dataset.theme = app.settings.theme;
    document.documentElement.dataset.accent = app.settings.accent;
  });
</script>

{#if splash}
  <Splash done={() => (splash = false)} />
{/if}

<div class="shell">
  <Sidebar bind:view />
  <main>
    {#key view}
      <div class="view" in:fly={{ y: 10, duration: 220, easing: cubicOut }}>
        {#if view === "library"}
          <LibraryView />
        {:else if view === "releases"}
          <ReleasesView />
        {:else if view === "activity"}
          <ActivityView />
        {:else}
          <SettingsView />
        {/if}
      </div>
    {/key}
  </main>

  {#if selectedGame}
    <GameDetail game={selectedGame} onclose={() => (app.selectedGameId = null)} />
  {/if}

  <Toasts />
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 216px 1fr;
    height: 100%;
    overflow: hidden;
  }
  main {
    overflow-y: auto;
    padding: 28px 32px 48px;
  }
</style>
