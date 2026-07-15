<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/stores.svelte";
  import { updater } from "./lib/updater.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import LibraryView from "./lib/components/LibraryView.svelte";
  import ReleasesView from "./lib/components/ReleasesView.svelte";
  import ActivityView from "./lib/components/ActivityView.svelte";
  import SettingsView from "./lib/components/SettingsView.svelte";
  import GameDetail from "./lib/components/GameDetail.svelte";
  import Toasts from "./lib/components/Toasts.svelte";

  let view = $state<"library" | "releases" | "activity" | "settings">("library");

  const selectedGame = $derived(
    app.library.games.find((g) => g.id === app.selectedGameId) ?? null
  );

  onMount(() => {
    app.init();
    updater.init();
  });
</script>

<div class="shell">
  <Sidebar bind:view />
  <main>
    {#if view === "library"}
      <LibraryView />
    {:else if view === "releases"}
      <ReleasesView />
    {:else if view === "activity"}
      <ActivityView />
    {:else}
      <SettingsView />
    {/if}
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
