<script lang="ts">
  // Launch splash: the logo's two PCB traces draw themselves across the
  // screen, solder pads pop in along the way, then the wordmark settles.
  // Purely decorative — removed from the DOM when done.
  let { done }: { done: () => void } = $props();

  // trace draw 700ms → pads pop → wordmark 500ms → hold → fade 350ms
  const TOTAL = 2100;
  setTimeout(done, TOTAL);
</script>

<div class="splash" style="--total: {TOTAL}ms">
  <div class="art">
    <svg viewBox="0 0 320 120" width="320" height="120" aria-hidden="true">
      <!-- top trace, arrow → -->
      <g class="trace top">
        <line x1="20" y1="35" x2="270" y2="35" />
        <circle class="p1" cx="55" cy="35" r="7" />
        <circle class="p2" cx="150" cy="35" r="7" />
        <polygon class="arrow" points="270,24 296,35 270,46" />
      </g>
      <!-- bottom trace, arrow ← -->
      <g class="trace bottom">
        <line x1="300" y1="85" x2="50" y2="85" />
        <circle class="p1" cx="205" cy="85" r="7" />
        <circle class="p2" cx="120" cy="85" r="7" />
        <polygon class="arrow" points="50,74 24,85 50,96" />
      </g>
    </svg>
    <span class="wordmark">UPLIFT</span>
  </div>
</div>

<style>
  .splash {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: var(--bg);
    animation: splash-out 350ms ease-in forwards;
    animation-delay: calc(var(--total) - 350ms);
  }
  @keyframes splash-out {
    to { opacity: 0; visibility: hidden; }
  }
  .art {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .trace line {
    stroke: var(--copper);
    stroke-width: 4;
    stroke-linecap: round;
    stroke-dasharray: 280;
    stroke-dashoffset: 280;
    animation: draw 700ms cubic-bezier(0.6, 0, 0.2, 1) forwards;
  }
  .trace.bottom line {
    stroke: var(--copper-dim);
    animation-delay: 140ms;
  }
  @keyframes draw {
    to { stroke-dashoffset: 0; }
  }

  .trace circle {
    fill: var(--bg);
    stroke: var(--copper);
    stroke-width: 4;
    transform-box: fill-box;
    transform-origin: center;
    transform: scale(0);
    animation: pop 260ms cubic-bezier(0.2, 1.4, 0.4, 1) forwards;
  }
  .trace.bottom circle { stroke: var(--copper-dim); }
  .trace.top .p1 { animation-delay: 260ms; }
  .trace.top .p2 { animation-delay: 420ms; }
  .trace.bottom .p1 { animation-delay: 480ms; }
  .trace.bottom .p2 { animation-delay: 620ms; }
  @keyframes pop {
    to { transform: scale(1); }
  }

  .trace .arrow {
    fill: var(--copper);
    opacity: 0;
    transform-box: fill-box;
    transform-origin: center;
    transform: translateX(-8px);
    animation: arrive 300ms ease-out forwards;
    animation-delay: 640ms;
  }
  .trace.bottom .arrow {
    fill: var(--copper-dim);
    transform: translateX(8px);
    animation-delay: 780ms;
  }
  @keyframes arrive {
    to { opacity: 1; transform: translateX(0); }
  }

  .wordmark {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 600;
    color: var(--ink);
    letter-spacing: 0.9em;
    padding-left: 0.9em; /* optically centre the tracked-out text */
    opacity: 0;
    animation: settle 500ms ease-out forwards;
    animation-delay: 900ms;
  }
  @keyframes settle {
    to { opacity: 1; letter-spacing: 0.32em; padding-left: 0.32em; }
  }

  @media (prefers-reduced-motion: reduce) {
    .splash { display: none; }
  }
</style>
