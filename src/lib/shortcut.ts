// Turning key presses into shortcut accelerators ("Ctrl+Alt+KeyN") and showing them ("Ctrl+Alt+N").
// The accelerator uses KeyboardEvent.code names (layout independent), which the Rust side parses.

export interface KeyInput {
  code: string
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  metaKey: boolean
}

export type Recorded =
  /** A complete, usable combination. */
  | { kind: 'accept'; accelerator: string }
  /** Esc: stop recording, keep what was there. */
  | { kind: 'cancel' }
  /** Backspace / Delete: stop recording and remove the shortcut. */
  | { kind: 'clear' }
  /** Only modifiers so far: keep listening. */
  | { kind: 'wait' }
  /** A plain key (it would be stolen from every app): refuse. */
  | { kind: 'reject' }

const MODIFIER_CODES = new Set([
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
  'MetaLeft',
  'MetaRight',
])

export function isFunctionKey(code: string): boolean {
  const m = /^F(\d{1,2})$/.exec(code)
  return m !== null && Number(m[1]) >= 1 && Number(m[1]) <= 24
}

/** What a key press means to the recorder. */
export function interpret(e: KeyInput): Recorded {
  if (MODIFIER_CODES.has(e.code)) return { kind: 'wait' }
  const anyModifier = e.ctrlKey || e.altKey || e.shiftKey || e.metaKey
  if (!anyModifier && e.code === 'Escape') return { kind: 'cancel' }
  if (!anyModifier && (e.code === 'Backspace' || e.code === 'Delete')) return { kind: 'clear' }
  if (!anyModifier && !isFunctionKey(e.code)) return { kind: 'reject' }
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  if (e.metaKey) parts.push('Super')
  parts.push(e.code)
  return { kind: 'accept', accelerator: parts.join('+') }
}

const KEY_NAMES: Record<string, string> = {
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Escape: 'Esc',
  Enter: 'Enter',
  Space: 'Space',
}

function keyName(code: string): string {
  if (code in KEY_NAMES) return KEY_NAMES[code]
  if (/^Key[A-Z]$/.test(code)) return code.slice(3)
  if (/^Digit\d$/.test(code)) return code.slice(5)
  if (/^Numpad\d$/.test(code)) return `Num ${code.slice(6)}`
  return code
}

/** "Ctrl+Alt+KeyN" -> "Ctrl+Alt+N" (or "⌃⌥N" on a Mac). */
export function display(accelerator: string, mac = false): string {
  const parts = accelerator.split('+')
  const key = keyName(parts[parts.length - 1])
  const mods = parts.slice(0, -1)
  if (mac) {
    const symbols: Record<string, string> = { Ctrl: '⌃', Alt: '⌥', Shift: '⇧', Super: '⌘' }
    const order = ['Ctrl', 'Alt', 'Shift', 'Super']
    return order.filter((m) => mods.includes(m)).map((m) => symbols[m]).join('') + key
  }
  const names: Record<string, string> = { Super: 'Win' }
  return [...mods.map((m) => names[m] ?? m), key].join('+')
}
