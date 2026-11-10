import { describe, expect, it } from 'vitest'
import type { Snapshot } from './bindings/Snapshot'
import {
  batteryTone,
  gainFromPosition,
  gainLabel,
  hzLabel,
  namesFitAsButtons,
  prefersDarkText,
  subtitle,
} from './format'

const snap = (over: Partial<Snapshot>): Snapshot => ({ link: 'connected', mode: null, level: null, budsBattery: null, ...over }) as Snapshot

describe('subtitle', () => {
  it('shows battery, mode and level', () => {
    expect(subtitle(snap({ mode: 'anc', level: 'max', budsBattery: 80 }))).toBe('80% · ANC · Max')
    expect(subtitle(snap({ mode: 'transparency', budsBattery: 12 }))).toBe('12% · Transparency')
    expect(subtitle(snap({ mode: 'off' }))).toBe('Off')
  })
  it('handles the link states', () => {
    expect(subtitle(snap({ link: 'connecting' }))).toBe('Connecting…')
    expect(subtitle(snap({ link: 'searching' }))).toBe('Not connected')
    expect(subtitle(snap({ mode: null, budsBattery: 50 }))).toBe('50% · Connected')
  })
  it('ignores a stale level outside ANC', () => {
    expect(subtitle(snap({ mode: 'off', level: 'max', budsBattery: 70 }))).toBe('70% · Off')
  })
})

describe('batteryTone', () => {
  it('is red at 20% or less unless charging, green when charging', () => {
    expect(batteryTone({ level: 20, charging: false })).toBe('low')
    expect(batteryTone({ level: 21, charging: false })).toBe('normal')
    expect(batteryTone({ level: 5, charging: true })).toBe('charging')
    expect(batteryTone(null)).toBe('normal')
  })
})

describe('labels', () => {
  it('formats gains and frequencies', () => {
    expect(gainLabel(3)).toBe('+3')
    expect(gainLabel(0)).toBe('0')
    expect(gainLabel(-2)).toBe('-2')
    expect(hzLabel(62)).toBe('62')
    expect(hzLabel(1000)).toBe('1k')
    expect(hzLabel(16000)).toBe('16k')
    expect(hzLabel(1500)).toBe('1.5k')
  })
})

describe('prefersDarkText', () => {
  it('picks readable text for accent colours', () => {
    expect(prefersDarkText('#0067C0')).toBe(false) // Windows blue: white text
    expect(prefersDarkText('#FFB900')).toBe(true) // yellow: black text
    expect(prefersDarkText('#000000')).toBe(false)
    expect(prefersDarkText('nonsense')).toBe(false)
  })
})

describe('namesFitAsButtons', () => {
  it('needs at most four names whose widths fit', () => {
    expect(namesFitAsButtons([50, 40, 30, 50], 250)).toBe(true) // 170 + 64 = 234
    expect(namesFitAsButtons([90, 90, 90], 250)).toBe(false)
    expect(namesFitAsButtons([10, 10, 10, 10, 10], 250)).toBe(false) // five names
    expect(namesFitAsButtons([], 250)).toBe(false)
  })
})

describe('gainFromPosition', () => {
  it('maps the top of the track to max and the bottom to min', () => {
    expect(gainFromPosition(0, 64, -6, 6)).toBe(6)
    expect(gainFromPosition(64, 64, -6, 6)).toBe(-6)
    expect(gainFromPosition(32, 64, -6, 6)).toBe(0)
    expect(gainFromPosition(-10, 64, -6, 6)).toBe(6)
    expect(gainFromPosition(500, 64, -6, 6)).toBe(-6)
  })
})
