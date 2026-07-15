export type Family = "dlss" | "dlss_g" | "dlss_d" | "xess" | "xess_fg" | "fsr";
export type Platform =
  | "steam" | "epic" | "gog" | "ubisoft" | "battle_net" | "xbox" | "manual";

export interface InstalledDll {
  family: Family;
  path: string;
  file_name: string;
  version: string;
  has_backup: boolean;
}

export interface GamePrefs {
  auto_update: boolean;
  pins: Record<string, string>;
}

export interface Game {
  id: number;
  name: string;
  platform: Platform;
  install_dir: string;
  steam_appid: number | null;
  cover_url: string | null;
  dlls: InstalledDll[];
  prefs: GamePrefs;
  anticheat: string | null; // "warn:EAC" | "block:Vanguard" | null
}

export interface Release {
  family: Family;
  version: string;
  url: string;
  sha256: string;
  zip_path: string | null;
  release_date: string | null;
  notified: boolean;
  downloaded: boolean;
}

export interface ChangelogEntry {
  family: Family;
  version: string;
  release_date: string | null;
  changes: string[];
  known_issues: string[];
  recommended_preset: string | null;
}

export interface SwapRecord {
  id: number;
  game_id: number;
  family: Family;
  dll_path: string;
  from_version: string;
  to_version: string;
  at: string;
  automatic: boolean;
}

export interface Settings {
  poll_hours: number;
  minimize_to_tray: boolean;
  notify_on_new_release: boolean;
  launch_at_startup: boolean;
}

export interface LibraryPayload {
  games: Game[];
  releases: Release[];
  changelogs: ChangelogEntry[];
  swaps: SwapRecord[];
}

export const FAMILY_LABEL: Record<Family, string> = {
  dlss: "DLSS",
  dlss_g: "Frame Generation",
  dlss_d: "Ray Reconstruction",
  xess: "XeSS",
  xess_fg: "XeSS Frame Gen",
  fsr: "FSR",
};

export const FAMILY_SHORT: Record<Family, string> = {
  dlss: "DLSS",
  dlss_g: "FG",
  dlss_d: "RR",
  xess: "XeSS",
  xess_fg: "X-FG",
  fsr: "FSR",
};

export const PLATFORM_LABEL: Record<Platform, string> = {
  steam: "Steam",
  epic: "Epic Games",
  gog: "GOG",
  ubisoft: "Ubisoft Connect",
  battle_net: "Battle.net",
  xbox: "Xbox",
  manual: "Manual",
};

export function versionCmp(a: string, b: string): number {
  const pa = a.split(/[.,-]/).map((n) => parseInt(n) || 0);
  const pb = b.split(/[.,-]/).map((n) => parseInt(n) || 0);
  const n = Math.max(pa.length, pb.length);
  for (let i = 0; i < n; i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Latest known release version for a family, or null. */
export function latestFor(releases: Release[], family: Family): Release | null {
  const list = releases.filter((r) => r.family === family);
  if (!list.length) return null;
  return list.reduce((a, b) => (versionCmp(a.version, b.version) >= 0 ? a : b));
}

/** Box-art candidates, best first. The backend-resolved URL tracks Steam's
 *  current art (seasonal capsules etc.); the flat CDN path is a stale-but-
 *  usually-present fallback for before the first resolve completes. */
export function coverUrls(game: Game): string[] {
  const urls: string[] = [];
  if (game.cover_url) urls.push(game.cover_url);
  if (game.steam_appid) {
    urls.push(
      `https://cdn.cloudflare.steamstatic.com/steam/apps/${game.steam_appid}/library_600x900.jpg`
    );
  }
  return urls;
}
