// Typed wrappers around the Tauri commands (src-tauri/src/commands.rs). Argument names are camelCase
// here; Tauri maps them to the snake_case Rust parameters.

import { invoke } from './tauri'
import type { AncLevel } from './bindings/AncLevel'
import type { EarSide } from './bindings/EarSide'
import type { Gesture } from './bindings/Gesture'
import type { GestureAction } from './bindings/GestureAction'
import type { LogEntry } from './bindings/LogEntry'
import type { NoiseMode } from './bindings/NoiseMode'
import type { Settings } from './bindings/Settings'
import type { Snapshot } from './bindings/Snapshot'

export const api = {
  getState: () => invoke<Snapshot>('get_state'),
  getSettings: () => invoke<Settings>('get_settings'),
  /** Merges a subset of the settings (camelCase keys) and saves. */
  updateSettings: (patch: Partial<Settings>) => invoke<void>('update_settings', { patch }),

  setMode: (mode: NoiseMode, level: AncLevel | null = null) => invoke<void>('set_mode', { mode, level }),
  cycleMode: () => invoke<void>('cycle_mode'),
  setNoiseBit: (bit: number) => invoke<void>('set_noise_bit', { bit }),
  setLowLatency: (on: boolean) => invoke<void>('set_low_latency', { on }),
  setEq: (id: number) => invoke<void>('set_eq', { id }),
  setCustomEq: (gains: number[]) => invoke<void>('set_custom_eq', { gains }),
  findBuds: (start: boolean) => invoke<void>('find_buds', { start }),
  setGesture: (side: EarSide, gesture: Gesture, action: GestureAction) =>
    invoke<void>('set_gesture', { side, gesture, action }),
  reconnect: () => invoke<void>('reconnect'),

  sendRaw: (command: number, payloadHex: string) => invoke<void>('send_raw', { command, payloadHex }),
  scanQueries: () => invoke<void>('scan_queries'),
  getLog: () => invoke<LogEntry[]>('get_log'),
  clearLog: () => invoke<void>('clear_log'),
  setLogPaused: (paused: boolean) => invoke<void>('set_log_paused', { paused }),
  exportLog: () => invoke<string>('export_log'),

  getAccentColor: () => invoke<string | null>('get_accent_color'),
  flyoutResize: (height: number) => invoke<void>('flyout_resize', { height }),
  flyoutHide: () => invoke<void>('flyout_hide'),
  /** Tell the flyout a native menu is open, so losing focus to it doesn't hide the flyout. */
  setMenuOpen: (open: boolean) => invoke<void>('set_menu_open', { open }),
  openSettings: () => invoke<void>('open_settings'),
  quit: () => invoke<void>('quit'),
  isFake: () => invoke<boolean>('is_fake'),
}
