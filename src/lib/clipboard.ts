// Copy text to the clipboard: the Tauri plugin in the app, the browser API in a plain browser.

import { isTauri } from './tauri'

export async function copyText(text: string): Promise<void> {
  if (isTauri) {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(text)
  } else {
    await navigator.clipboard.writeText(text)
  }
}
