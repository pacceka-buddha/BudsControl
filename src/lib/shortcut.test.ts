import { describe, expect, it } from 'vitest'
import { display, interpret, isFunctionKey } from './shortcut'

const key = (code: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {}) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
})

describe('interpret', () => {
  it('accepts a key with modifiers, in a fixed modifier order', () => {
    expect(interpret(key('KeyN', { altKey: true, ctrlKey: true }))).toEqual({ kind: 'accept', accelerator: 'Ctrl+Alt+KeyN' })
    expect(interpret(key('Digit1', { shiftKey: true, metaKey: true, ctrlKey: true }))).toEqual({
      kind: 'accept',
      accelerator: 'Ctrl+Shift+Super+Digit1',
    })
  })
  it('keeps listening while only modifiers are held', () => {
    expect(interpret(key('ControlLeft', { ctrlKey: true }))).toEqual({ kind: 'wait' })
    expect(interpret(key('ShiftRight', { shiftKey: true }))).toEqual({ kind: 'wait' })
    expect(interpret(key('MetaLeft', { metaKey: true }))).toEqual({ kind: 'wait' })
  })
  it('lets Esc cancel and Backspace / Delete clear, but only without modifiers', () => {
    expect(interpret(key('Escape'))).toEqual({ kind: 'cancel' })
    expect(interpret(key('Backspace'))).toEqual({ kind: 'clear' })
    expect(interpret(key('Delete'))).toEqual({ kind: 'clear' })
    expect(interpret(key('Backspace', { ctrlKey: true }))).toEqual({ kind: 'accept', accelerator: 'Ctrl+Backspace' })
  })
  it('refuses plain keys (they would be stolen from every app) except function keys', () => {
    expect(interpret(key('KeyA'))).toEqual({ kind: 'reject' })
    expect(interpret(key('Space'))).toEqual({ kind: 'reject' })
    expect(interpret(key('F9'))).toEqual({ kind: 'accept', accelerator: 'F9' })
    expect(interpret(key('F24'))).toEqual({ kind: 'accept', accelerator: 'F24' })
  })
})

describe('isFunctionKey', () => {
  it('knows F1 to F24 only', () => {
    expect(isFunctionKey('F1')).toBe(true)
    expect(isFunctionKey('F12')).toBe(true)
    expect(isFunctionKey('F24')).toBe(true)
    expect(isFunctionKey('F25')).toBe(false)
    expect(isFunctionKey('F0')).toBe(false)
    expect(isFunctionKey('KeyF')).toBe(false)
  })
})

describe('display', () => {
  it('shows readable key names on Windows / Linux', () => {
    expect(display('Ctrl+Alt+KeyN')).toBe('Ctrl+Alt+N')
    expect(display('Ctrl+Shift+Digit1')).toBe('Ctrl+Shift+1')
    expect(display('Alt+ArrowUp')).toBe('Alt+↑')
    expect(display('Ctrl+Numpad5')).toBe('Ctrl+Num 5')
    expect(display('Super+Space')).toBe('Win+Space')
    expect(display('F9')).toBe('F9')
    expect(display('Ctrl+Backquote')).toBe('Ctrl+`')
  })
  it('uses symbols on a Mac', () => {
    expect(display('Ctrl+Alt+Shift+Super+KeyN', true)).toBe('⌃⌥⇧⌘N')
    expect(display('Super+Digit4', true)).toBe('⌘4')
  })
})
