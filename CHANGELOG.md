# Changelog

All notable changes to Uplift. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [0.4.0] - 2026-07-15

### Added
- **Crash detection + automatic rollback** — after any swap (manual, auto, or
  re-apply) Uplift watches the game's next sessions. Two quick crashes in a
  row automatically restore every backed-up DLL, clear the remembered
  version so re-apply doesn't redo the damage, and notify you. One healthy
  session (2½ minutes) ends the watch.
- **Before/after proof** — a Proof section in every game panel captures 30
  seconds of real frametimes via Intel's PresentMon (downloaded once from the
  official release, hash-verified) and stores avg fps and 1% lows. Benchmark
  before and after a swap or preset change and the panel shows the delta —
  numbers instead of feelings.
- **Themes** — dark and light, with four accent palettes (copper, mint,
  azure, violet). Applies instantly, persists, lives in Settings →
  Appearance.
- **Launch animation** — the logo's PCB traces draw themselves across the
  screen, solder pads pop in, the wordmark settles, and the app fades in.
  Views now glide when switching tabs. Both respect reduced-motion settings.

## [0.3.0] - 2026-07-15

### Added
- **Swap re-apply after game updates** — game patches silently restore stock
  DLLs; Uplift now detects this (checking real on-disk versions in the
  background cycle and on every rescan) and re-applies the version you chose.
  Per game, on by default, skipped for anti-cheat-flagged or running games.
  You get a notification when it happens.
- **DLSS indicator toggle** (Settings) — one click enables NVIDIA's on-screen
  NGX overlay in every DLSS game, showing the loaded DLL version, render
  preset and mode, so you can verify a swap or preset override is really
  active. System-wide; toggling asks for administrator approval.

### Fixed
- Version rail: the copper trace now spans the whole scrollable rail instead
  of stopping at the visible edge, the rail shows the full version history
  (the previous build silently capped it at 8), and it opens centred on your
  installed version.
- Auto-update now applies the same filename guard as manual swaps (an FSR
  DX12 build can no longer be auto-swapped onto a Vulkan DLL) and keeps the
  library database in sync after each automatic swap.

## [0.2.0] - 2026-07-15

### Added
- **Per-game driver preset overrides** — force a DLSS or Ray Reconstruction
  render preset (transformer K/J, CNN A–F, or "Latest") into the NVIDIA
  driver profile for each game, the same mechanism NVIDIA Profile Inspector
  uses. Every preset has a short description of what sets it apart, and the
  community-recommended preset is preselected by default.
- **Full DLL archive** — the release manifest now lists every build in the
  community archive (161 releases: 77 DLSS, 29 Frame Generation, 15 Ray
  Reconstruction, 15 XeSS, 9 XeSS Frame Gen, 16 FSR), each verified with the
  SHA-256 of the final DLL. Every version of every family is downloadable,
  for upgrades and downgrades alike.

### Changed
- The Super Resolution family is now labelled **DLSS** everywhere (cards,
  game panel, notifications) — the term everyone actually uses.

## [0.1.0] - 2026-07-15

Initial release.

- Scans Steam, Epic Games, GOG, Ubisoft Connect, Battle.net, the Xbox app and
  manual folders for swappable DLSS/FSR/XeSS DLLs, reading true versions from
  PE headers.
- Safe swapping: SHA-256-verified downloads, first-swap backups with
  one-click restore, refuse-if-running, filename matching, anti-cheat
  block/warn list.
- Tray-resident background loop: release notifications and guarded per-game
  auto-update.
- Community-curated changelog database rendered per version, with full diffs
  between installed and target versions.
- Live Steam box art (current seasonal capsules via the store API).
- Signed in-app self-updater fed from GitHub releases.
