// Reactive app state shared by every window. `initStores()` loads the current values and then keeps
// them current from the events the Rust side emits (`buds://state`, `buds://settings`, `buds://log`).

import { listen } from './tauri'
import { api } from './api'
import type { LogEntry } from './bindings/LogEntry'
import type { PlatformInfo } from './bindings/PlatformInfo'
import type { Settings } from './bindings/Settings'
import type { Snapshot } from './bindings/Snapshot'

/** Same cap as the Rust ring buffer. */
const LOG_LIMIT = 600

export const app = $state<{
  snapshot: Snapshot | null
  settings: Settings | null
  log: LogEntry[]
  fake: boolean
  /** Actions (`cycle`, `anc`, ...) whose shortcut another program already owns. */
  conflicts: string[]
  /** The OS the app runs on (null until loaded). */
  platform: PlatformInfo | null
}>({ snapshot: null, settings: null, log: [], fake: false, conflicts: [], platform: null })

let started = false

export async function initStores(): Promise<void> {
  if (started) return
  started = true
  // Subscribe before reading so nothing is missed in between.
  await Promise.all([
    listen<Snapshot>('buds://state', (payload) => {
      app.snapshot = payload
    }),
    listen<Settings>('buds://settings', (payload) => {
      app.settings = payload
    }),
    listen<string[]>('buds://conflicts', (payload) => {
      app.conflicts = payload
    }),
    listen<LogEntry[]>('buds://log', (payload) => {
      app.log = [...app.log, ...payload].slice(-LOG_LIMIT)
    }),
  ])
  const [snapshot, settings, log, fake, conflicts, platform] = await Promise.all([
    api.getState(),
    api.getSettings(),
    api.getLog(),
    api.isFake(),
    api.getConflicts(),
    api.getPlatform(),
  ])
  app.platform = platform
  app.conflicts = conflicts
  app.snapshot = snapshot
  app.settings = settings
  app.log = log
  app.fake = fake
}

/** Empties the packet list shown in the Developer tab (after the Rust side was cleared). */
export function resetLog(): void {
  app.log = []
}
