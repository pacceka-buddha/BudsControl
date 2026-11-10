// The only place that talks to Tauri. Outside the app (a plain browser, for design review and
// screenshots) the same calls are answered by the simulator in mock.ts, so the UI can be developed
// and checked in every state without the app or the buds.

import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event'
import * as mock from './mock'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri ? tauriInvoke<T>(command, args) : (mock.invoke(command, args) as Promise<T>)
}

export function listen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return isTauri
    ? tauriListen<T>(event, (e) => handler(e.payload))
    : mock.listen<T>(event, handler)
}
