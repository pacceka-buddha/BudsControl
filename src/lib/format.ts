// Small pure helpers for turning controller state into text and colours. Unit-tested (format.test.ts).

import type { AncLevel } from './bindings/AncLevel'
import type { Battery } from './bindings/Battery'
import type { NoiseMode } from './bindings/NoiseMode'
import type { Snapshot } from './bindings/Snapshot'

export const MODE_TITLE: Record<NoiseMode, string> = {
  off: 'Off',
  transparency: 'Transparency',
  anc: 'Noise Cancellation',
}

export const MODE_SHORT: Record<NoiseMode, string> = {
  off: 'Off',
  transparency: 'Transparency',
  anc: 'ANC',
}

export const LEVEL_TITLE: Record<AncLevel, string> = {
  smart: 'Smart',
  max: 'Max',
  moderate: 'Moderate',
  mild: 'Mild',
  adaptive: 'Adaptive',
}

/** Below this, a battery that isn't charging is shown in red. */
export const LOW_BATTERY = 20

/** "80% · ANC · Max", "Connecting…", "Not connected": the line under the device name. */
export function subtitle(s: Snapshot): string {
  if (s.link === 'connecting') return 'Connecting…'
  if (s.link === 'searching') return 'Not connected'
  let status = s.mode ? MODE_SHORT[s.mode] : 'Connected'
  if (s.mode === 'anc' && s.level) status += ` · ${LEVEL_TITLE[s.level]}`
  return s.budsBattery == null ? status : `${s.budsBattery}% · ${status}`
}

export type BatteryTone = 'normal' | 'low' | 'charging'

/** Red when low and not charging, green when charging. */
export function batteryTone(b: Battery | null | undefined): BatteryTone {
  if (!b) return 'normal'
  if (b.charging) return 'charging'
  return b.level <= LOW_BATTERY ? 'low' : 'normal'
}

/** "+3", "0", "-2": the dB label above an EQ band. */
export function gainLabel(gain: number): string {
  return gain > 0 ? `+${gain}` : `${gain}`
}

/** "62", "250", "1k", "16k": the frequency label under an EQ band. */
export function hzLabel(hz: number): string {
  return hz >= 1000 ? `${hz / 1000}k` : `${hz}`
}

/** `#RRGGBB` -> whether black text is more readable on it than white (WCAG relative luminance). */
export function prefersDarkText(hex: string): boolean {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex)
  if (!m) return false
  const n = parseInt(m[1], 16)
  const channel = (v: number) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  const lum = 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255)
  return lum > 0.4
}

/** Whether labels fit side by side as segments: at most four, and they sum to at most `maxPx`. */
export function namesFitAsButtons(widths: number[], maxPx: number, padPerButton = 16): boolean {
  if (widths.length > 4 || widths.length === 0) return false
  return widths.reduce((sum, w) => sum + w + padPerButton, 0) <= maxPx
}

/** Clamp and round a pointer position inside a vertical track to a gain in [min, max]. */
export function gainFromPosition(y: number, height: number, min: number, max: number): number {
  const fraction = Math.min(Math.max(y / height, 0), 1)
  return max - Math.round(fraction * (max - min))
}
