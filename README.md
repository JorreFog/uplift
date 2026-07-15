# Uplift

A modern manager for **DLSS, FSR and XeSS DLLs** — scan your games, see exactly
which DLSS / Frame Generation / Ray Reconstruction builds they run, swap
versions safely, get notified when new builds release, and (per game)
auto-update.

Built with **Tauri 2 (Rust)** + **Svelte 5**. Windows-first.

## Install

Download the latest `Uplift_x.y.z_x64-setup.exe` (NSIS) or `.msi` from
[Releases](https://github.com/JorreFog/uplift/releases) and run it. The app
keeps itself up to date: when a new version is published on GitHub, Uplift
notifies you and installs it in one click from Settings.

## What it does

- **Scans launchers** — Steam, Epic Games, GOG, Ubisoft Connect, Battle.net,
  Xbox app, plus manually added folders — and finds every swappable DLL
  (`nvngx_dlss`, `nvngx_dlssg`, `nvngx_dlssd`, `libxess`, `libxess_fg`, FSR
  DLLs), reading the real version from each file's PE header.
- **Current box art** — covers are resolved live from Steam's store API, so
  seasonal art changes (hello, Diablo IV) show up automatically.
- **Release notifications** — a tray-resident background loop polls the
  releases manifest and fires a Windows toast when a new build drops,
  telling you how many of your games can be upgraded.
- **Auto-update, guarded** — opt-in per game; never touches pinned versions,
  anti-cheat-flagged games, or games that are currently running; always backs
  up the original DLL first and verifies the swap landed.
- **Changelogs** — a community-curated changelog database renders per-version
  notes, and the game panel stitches a full diff of everything that changed
  between your installed version and the one you're about to swap to.
- **Version rail** — each DLL family is drawn as a copper trace with a solder
  pad per release: your installed pad glows, downloaded builds carry a dot,
  and clicking a pad plans a swap or downgrade.
- **Safety rails** — SHA-256 verification of every download against the
  manifest, sacred first-swap backups (`*.uplift.bak`) with one-click restore,
  refuse-if-running, filename matching so a DX12 FSR build can never land on
  a Vulkan DLL, and a block/warn anti-cheat list (Vanguard, RICOCHET and
  friends are hard-blocked).

## Building from source

Prereqs: [Rust](https://rustup.rs), Node 20+, and the
[Tauri 2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/)
(WebView2 + MSVC build tools).

```bash
npm install
npm run tauri dev      # develop
npm run tauri build    # produce NSIS/MSI installers
cd src-tauri && cargo test   # includes an end-to-end download→swap test
```

Release builds sign updater artifacts; set `TAURI_SIGNING_PRIVATE_KEY` to the
contents of your updater private key first (see Releasing below).

## Releasing

One-time setup:

1. Create the GitHub repo `JorreFog/uplift` and push this tree.
2. Create a release with tag **`dll-archive`**, tick **pre-release** (so it
   never shadows app releases as "latest"), and upload every zip from the
   local `dll-archive/` folder as assets. `manifest/dll-releases.json` already
   points at those asset URLs.
3. Add the repository secret **`TAURI_SIGNING_PRIVATE_KEY`** with the contents
   of `~/.tauri/uplift.key` (generated once with
   `npm run tauri signer generate`; keep the file safe — lose it and shipped
   apps can no longer accept updates).

Each release after that:

```bash
git tag v0.2.0 && git push --tags
```

CI (`.github/workflows/release.yml`) builds, signs and publishes the
installers plus the `latest.json` updater feed; running apps pick the update
up automatically.

## The manifest

The app polls three JSON files from this repo's `manifest/` folder (raw
GitHub URLs, baked in `src-tauri/src/remote.rs`):

- `dll-releases.json` — one entry per downloadable DLL build: direct `url`,
  `sha256` **of the final DLL file** (the app refuses mismatches), `zip_path`
  when the DLL sits inside a zip. The zips themselves are assets of the
  `dll-archive` release.
- `changelogs.json` — community-curated per-version notes. NVIDIA publishes
  no official per-DLL changelogs, so this file is the product's editorial
  layer; PRs welcome.
- `anticheat.json` — block/warn list matched on name substrings and Steam
  appids.

To publish a new DLL build: upload the zip to the `dll-archive` release, add
an entry with the DLL's SHA-256 to `dll-releases.json`, push to `main`.
Clients pick it up on their next poll and notify.

## Architecture

```
src-tauri/src/
  models.rs      shared types, DLL family definitions, version comparison
  db.rs          SQLite (games, dlls, releases, downloads, pins, swaps, settings)
  scanners/      steam (hand-rolled VDF parser), epic, gog, ubisoft,
                 battlenet, xbox (.GamingRoot) — all best-effort, all pruned
  dll.rs         walkdir discovery + PE version via pelite
  remote.rs      releases / changelogs / anticheat manifests + Steam box art
  downloads.rs   fetch → unzip if needed → SHA-256 verify → library store
  swap.rs        refuse-if-running, first-swap backup, copy, post-verify
  background.rs  poll loop → toasts for new releases → guarded auto-update pass
  commands.rs    the invoke surface
  lib.rs         tray, hide-to-tray, autostart, self-updater, plugin wiring
  tests/         end-to-end pipeline test against the real archive zips
src/             Svelte 5 (runes) — Library grid, game slide-over with version
                 rails + changelog diffs, Releases browser, Activity feed,
                 Settings (incl. in-app updates)
manifest/        the community data files (schema in SCHEMA.md)
dll-archive/     staged DLL zips → upload as `dll-archive` release assets
                 (gitignored; not part of the repo history)
```

Design language: "hardware bench" — graphite panels, copper accents (PCB
traces), Chakra Petch display type, IBM Plex Sans body, IBM Plex Mono for
every version string. Fonts are bundled; the app is fully usable offline
except for downloads, manifest refresh, and Steam cover art.

## Roadmap (not yet implemented)

- **DLSS preset overrides** (force preset K/J per game) via NVAPI DRS —
  what NVIDIA Profile Inspector does; needs NVAPI bindings.
- **NVIDIA App override detection** — warn when the driver-level DLSS
  override is already active so the two don't fight.
- **Elevation flow** — per-swap "retry elevated" for protected directories
  instead of asking users to run the whole app as admin.
- **Xbox deep support** — some titles keep DLLs inside locked WindowsApps
  paths that even elevation can't cleanly reach.

## License

[GPL-3.0](LICENSE). Uplift is a from-scratch implementation inspired by
[DLSS Swapper](https://github.com/beeradmoore/dlss-swapper) (GPL-3.0); no code
was copied from it. DLL version/archive data is sourced from the community
(TechPowerUp, the DLSS Swapper archive). NVIDIA DLSS, Intel XeSS and AMD
FidelityFX DLLs remain the property of their respective owners and are
redistributed under their runtime redistribution terms.
