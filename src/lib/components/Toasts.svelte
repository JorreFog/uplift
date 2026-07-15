<script lang="ts">
  import { app } from "../stores.svelte";
</script>

<div class="stack" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}">{t.text}</div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    bottom: 18px;
    right: 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 50;
    max-width: 380px;
  }
  .toast {
    background: var(--panel-2);
    border: 1px solid var(--line-bright);
    border-left: 3px solid var(--mint);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 13px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    animation: pop 160ms ease-out;
    word-break: break-word;
  }
  .toast.err { border-left-color: var(--red); }
  @keyframes pop {
    from { transform: translateY(8px); opacity: 0; }
    to { transform: none; opacity: 1; }
  }
</style>
