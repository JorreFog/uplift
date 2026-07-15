import { invoke } from "@tauri-apps/api/core";
import type { GamePrefs, GamePresets, LibraryPayload, Settings } from "./types";

export const api = {
  getLibrary: () => invoke<LibraryPayload>("get_library"),
  scanGames: () => invoke<LibraryPayload>("scan_games"),
  addManualGame: (name: string, dir: string) =>
    invoke<LibraryPayload>("add_manual_game", { name, dir }),
  refreshRemote: () => invoke<string>("refresh_remote"),
  downloadDll: (family: string, version: string) =>
    invoke<LibraryPayload>("download_dll", { family, version }),
  swapDll: (gameId: number, family: string, version: string, confirmedAnticheat = false) =>
    invoke<LibraryPayload>("swap_dll", {
      gameId, family, version, confirmedAnticheat,
    }),
  restoreDll: (gameId: number, family: string) =>
    invoke<LibraryPayload>("restore_dll", { gameId, family }),
  setGamePrefs: (gameId: number, prefs: GamePrefs) =>
    invoke<void>("set_game_prefs", { gameId, prefs }),
  getGamePresets: (gameId: number) =>
    invoke<GamePresets>("get_game_presets", { gameId }),
  setGamePreset: (gameId: number, family: string, value: number) =>
    invoke<GamePresets>("set_game_preset", { gameId, family, value }),
  getDlssIndicator: () => invoke<boolean>("get_dlss_indicator"),
  setDlssIndicator: (enabled: boolean) =>
    invoke<boolean>("set_dlss_indicator", { enabled }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
};
