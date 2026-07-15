# Uplift manifest repo

Three JSON files the app polls (raw GitHub URLs configured in `src-tauri/src/remote.rs`):

## dll-releases.json
One entry per downloadable DLL build.
- `family`: `dlss` | `dlss_g` | `dlss_d` | `xess` | `xess_fg` | `fsr`
- `version`: dotted file version as reported by the DLL's PE header
- `url`: direct download (zip or bare dll)
- `sha256`: hash of the **final DLL file** (not the zip). The app refuses mismatches.
- `zip_path`: path of the DLL inside the zip, omit for bare files
- `release_date`: ISO date, optional

## changelogs.json
Community-curated notes per version. NVIDIA publishes no official DLL
changelogs, so this is the project's editorial layer.
- `changes`: short factual bullets
- `known_issues`: optional
- `recommended_preset`: optional letter (DLSS presets)

## anticheat.json
- match on `name_contains` (case-insensitive substring) and/or `steam_appid`
- `severity: "block"` — never swap; `"warn"` — confirm manual swaps, never auto-swap
