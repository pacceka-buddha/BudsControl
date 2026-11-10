// Stand-in for the Rust side when the UI runs in a plain browser (design review, screenshots).
// Open  http://localhost:1420/flyout.html?scenario=<name>  to pick a state (settings.html?tab=developer&dev=1
// shows the settings window):
//   connected (default) · anc · disconnected · connecting · lowbattery · charging · finding
//   longeq (EQ names too long for buttons) · minimal (no More panel) · noanc (buds without ANC)
// It only imitates what the screens need; the real behaviour lives in buds-core.

import type { ActionOption } from './bindings/ActionOption'
import type { AncLevel } from './bindings/AncLevel'
import type { EarSide } from './bindings/EarSide'
import type { Gesture } from './bindings/Gesture'
import type { GestureAction } from './bindings/GestureAction'
import type { GestureSlot } from './bindings/GestureSlot'
import type { NoiseMode } from './bindings/NoiseMode'
import type { Settings } from './bindings/Settings'
import type { SideGestures } from './bindings/SideGestures'
import type { Snapshot } from './bindings/Snapshot'

type Handler = (payload: never) => void
const listeners = new Map<string, Set<Handler>>()

export function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  const set = listeners.get(event) ?? new Set()
  set.add(handler as Handler)
  listeners.set(event, set)
  return Promise.resolve(() => set.delete(handler as Handler))
}

function emit(event: string, payload: unknown) {
  for (const h of listeners.get(event) ?? []) (h as (p: unknown) => void)(payload)
}

const ACTIONS: GestureAction[] = ['none', 'playPause', 'assistant', 'previous', 'next', 'volumeUp', 'volumeDown', 'noiseControl', 'lowLatency']
const TITLES: Record<GestureAction, string> = {
  none: 'None',
  playPause: 'Play / Pause',
  assistant: 'Voice assistant',
  assistantRealme: 'Voice assistant',
  previous: 'Previous track',
  next: 'Next track',
  noiseControl: 'Noise control',
  volumeUp: 'Volume up',
  volumeDown: 'Volume down',
  lowLatency: 'Low latency',
}

function slot(current: GestureAction): GestureSlot {
  const options: ActionOption[] = ACTIONS.map((action) => ({ action, title: TITLES[action] }))
  return { current, options }
}

function side(left: boolean): SideGestures {
  return {
    doubleTap: slot(left ? 'previous' : 'next'),
    tripleTap: slot('assistant'),
    hold: slot('noiseControl'),
  }
}

const BITS: Record<string, number> = { off: 3, transparency: 8, anc: 1, smart: 7, max: 4, moderate: 5, mild: 6, adaptive: 11 }

function base(): Snapshot {
  return {
    link: 'connected',
    deviceName: 'OnePlus Buds 4',
    productId: '065414',
    address: '00:A4:1C:39:41:4A',
    channel: 15,
    mode: 'transparency',
    level: null,
    noiseBit: 8,
    left: { level: 82, charging: false },
    right: { level: 80, charging: false },
    case: { level: 45, charging: false },
    budsBattery: 80,
    lowLatency: false,
    eqId: 0,
    customEqs: [],
    activeCustomEq: null,
    eqChoices: [
      { id: 0, name: 'Balanced' },
      { id: 1, name: 'Vocal' },
      { id: 2, name: 'Bass' },
      { id: 3, name: 'Custom' },
    ],
    gestures: { left: side(true), right: side(false) },
    findingBuds: false,
    hosts: [
      { address: 'AA:BB:CC:DD:EE:01', name: 'DESKTOP-5RT2K', connected: true, isThisHost: true },
      { address: '11:22:33:44:55:66', name: 'Pixel 9', connected: true, isThisHost: false },
    ],
    wearLeft: 'inEar',
    wearRight: 'inEar',
    capabilitiesCount: 35,
    modelName: 'OnePlus Buds 4',
    inCatalog: true,
    brand: 'oneplus',
    availableModes: ['off', 'transparency', 'anc'],
    availableLevels: ['smart', 'max', 'moderate', 'mild', 'adaptive'],
    supports: { noiseControl: true, equalizer: true, customEq: true, lowLatency: true, findBuds: true, gestures: true },
    hasMoreControls: true,
    layout: [
      { bit: 0, modeType: 1, name: 'Off' },
      { bit: 1, modeType: 5, name: 'ANC' },
      { bit: 2, modeType: 2, name: 'Transparency' },
      { bit: 3, modeType: 1, name: 'Off' },
      { bit: 4, modeType: 4, name: 'ANC · Max' },
      { bit: 5, modeType: 8, name: 'ANC · Moderate' },
      { bit: 6, modeType: 3, name: 'ANC · Mild' },
      { bit: 7, modeType: 7, name: 'ANC · Smart' },
      { bit: 8, modeType: 2, name: 'Transparency' },
      { bit: 11, modeType: 10, name: 'ANC · Adaptive' },
    ],
    hostLabel: 'PC',
  }
}

function customEq(): Snapshot['activeCustomEq'] {
  const hz = [62, 250, 1000, 4000, 8000, 16000]
  const gain = [2, 0, -1, 3, 1, -2]
  return { id: 3, name: 'Custom', minGain: -6, maxGain: 6, bands: hz.map((h, i) => ({ hz: h, gain: gain[i] })) }
}

function scenario(name: string): Snapshot {
  const s = base()
  switch (name) {
    case 'anc':
      s.mode = 'anc'
      s.level = 'max'
      s.noiseBit = 4
      break
    case 'customeq':
      s.eqId = 3
      s.customEqs = [customEq()!]
      s.activeCustomEq = customEq()
      s.mode = 'anc'
      s.level = 'smart'
      break
    case 'disconnected':
      Object.assign(s, { link: 'searching', mode: null, left: null, right: null, case: null, budsBattery: null, productId: null })
      s.supports = { noiseControl: false, equalizer: false, customEq: false, lowLatency: false, findBuds: false, gestures: false }
      s.hasMoreControls = false
      break
    case 'connecting':
      Object.assign(s, { link: 'connecting', mode: null, left: null, right: null, case: null, budsBattery: null })
      s.hasMoreControls = false
      break
    case 'lowbattery':
      s.left = { level: 12, charging: false }
      s.right = { level: 18, charging: false }
      s.case = { level: 5, charging: false }
      s.budsBattery = 12
      break
    case 'charging':
      s.left = { level: 64, charging: true }
      s.right = { level: 70, charging: true }
      s.case = { level: 90, charging: true }
      s.budsBattery = 64
      break
    case 'finding':
      s.findingBuds = true
      break
    case 'longeq':
      s.eqChoices = [
        { id: 0, name: 'Balanced' },
        { id: 1, name: 'Dynaudio Natural' },
        { id: 2, name: 'Pure vocals' },
        { id: 3, name: 'Big bass' },
        { id: 4, name: 'Dynaudio Bold' },
      ]
      break
    case 'minimal':
      s.deviceName = 'OPPO Enco Free'
      s.modelName = 'OPPO Enco Free'
      s.brand = 'oppo'
      s.mode = null
      s.supports = { noiseControl: false, equalizer: false, customEq: false, lowLatency: false, findBuds: false, gestures: false }
      s.hasMoreControls = false
      s.availableModes = []
      s.availableLevels = []
      s.case = null
      break
    case 'noanc':
      s.availableModes = ['off', 'transparency']
      s.availableLevels = []
      s.supports.noiseControl = true
      break
  }
  return s
}

const params = new URLSearchParams(typeof location === 'undefined' ? '' : location.search)
const state: Snapshot = scenario(params.get('scenario') ?? 'connected')
const settings: Settings = {
  autoConnect: true,
  rememberSettings: false,
  showHud: true,
  developerMode: params.get('dev') === '1',
  moreExpanded: params.get('more') === '1',
  gestureSide: 'left',
  lastAncLevel: null,
  lastBudsAddress: null,
  knownBuds: [],
  connectHints: {},
  shortcuts: {},
  remembered: {},
}

let autostart = false
let logPaused = false
type Dir = 'sent' | 'received' | 'info'
const log: { timeMs: number; direction: Dir; text: string }[] = ([
  { direction: 'info', text: 'connecting to OnePlus Buds 4 (00:A4:1C:39:41:4A)' },
  { direction: 'info', text: 'connected via HeyMelody service 079A' },
  { direction: 'sent', text: 'AA 07 00 00 00 01 01 00 00' },
  { direction: 'received', text: 'AA 10 00 00 00 81 01 09 00 00 FF 77 5A EA 67 0E 20 07' },
  { direction: 'sent', text: 'AA 07 00 00 03 01 02 00 00' },
  { direction: 'received', text: 'AA 0B 00 00 03 81 02 04 00 00 14 54 06' },
] as { direction: Dir; text: string }[]).map((e, i) => ({ ...e, timeMs: Date.now() - (6 - i) * 120 }))

function publish() {
  const left = state.left?.level
  const right = state.right?.level
  state.budsBattery = left == null && right == null ? null : Math.min(left ?? 100, right ?? 100)
  emit('buds://state', structuredClone(state))
}

export function invoke(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (command) {
    case 'get_state':
      return Promise.resolve(structuredClone(state))
    case 'get_settings':
      return Promise.resolve(structuredClone(settings))
    case 'update_settings':
      Object.assign(settings, args.patch)
      emit('buds://settings', structuredClone(settings))
      break
    case 'set_mode': {
      const mode = args.mode as NoiseMode
      state.mode = mode
      state.level = mode === 'anc' ? ((args.level as AncLevel | null) ?? state.level ?? 'smart') : null
      state.noiseBit = BITS[state.level ?? mode]
      publish()
      break
    }
    case 'set_low_latency':
      state.lowLatency = args.on as boolean
      publish()
      break
    case 'set_eq':
      state.eqId = args.id as number
      publish()
      break
    case 'set_custom_eq': {
      if (state.activeCustomEq) {
        const gains = args.gains as number[]
        state.activeCustomEq.bands.forEach((b, i) => (b.gain = gains[i] ?? b.gain))
        publish()
      }
      break
    }
    case 'find_buds':
      state.findingBuds = args.start as boolean
      publish()
      break
    case 'set_gesture': {
      const sideState = state.gestures[args.side as EarSide]
      const slotState = sideState?.[args.gesture as Gesture]
      if (slotState) slotState.current = args.action as GestureAction
      publish()
      break
    }
    case 'reconnect':
      state.link = 'connecting'
      publish()
      setTimeout(() => {
        Object.assign(state, scenario('connected'))
        publish()
      }, 1200)
      break
    case 'get_log':
      return Promise.resolve(structuredClone(log))
    case 'get_log_paused':
      return Promise.resolve(logPaused)
    case 'set_log_paused':
      logPaused = args.paused as boolean
      break
    case 'clear_log':
      log.length = 0
      break
    case 'export_log':
      return Promise.resolve(log.map((e) => `${new Date(e.timeMs).toISOString().slice(11, 23)} ${e.text}`).join('\n'))
    case 'get_autostart':
      return Promise.resolve(autostart)
    case 'set_autostart':
      autostart = args.enabled as boolean
      return Promise.resolve(autostart)
    case 'get_conflicts':
      return Promise.resolve(params.get('conflict') ? [params.get('conflict') as string] : [])
    case 'set_recording':
      break
    case 'validate_shortcut':
      return Promise.resolve(/^((Ctrl|Alt|Shift|Super)\+)*[A-Za-z0-9]+$/.test(args.accelerator as string))
    case 'get_hud_pending':
      return Promise.resolve(null)
    case 'hud_done':
      break
    case 'app_version':
      return Promise.resolve('0.1.0')
    case 'send_raw': {
      const hex = (n: number, w: number) => n.toString(16).toUpperCase().padStart(w, '0')
      const entry = { timeMs: Date.now(), direction: 'sent' as const, text: `AA 07 00 00 ${hex((args.command as number) & 255, 2)} ${hex((args.command as number) >> 8, 2)} 01 00 00` }
      log.push(entry)
      emit('buds://log', [entry])
      break
    }
    case 'get_platform': {
      const platform = params.get('platform')
      return Promise.resolve({ os: platform === 'linux' || platform === 'wayland' ? 'linux' : 'windows', wayland: platform === 'wayland' })
    }
    case 'get_accent_color':
      return Promise.resolve(params.get('accent') ? `#${params.get('accent')}` : null)
    case 'is_fake':
      return Promise.resolve(true)
    case 'flyout_resize':
      ;(window as unknown as { __flyoutHeight?: number }).__flyoutHeight = args.height as number
      break
    default:
      break
  }
  return Promise.resolve(undefined)
}
