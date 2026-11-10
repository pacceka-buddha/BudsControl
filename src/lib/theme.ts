// Applies the user's Windows accent colour to the page's design tokens.

import { api } from './api'
import { prefersDarkText } from './format'

export async function applyAccent(): Promise<void> {
  const hex = await api.getAccentColor().catch(() => null)
  if (!hex) return
  const root = document.documentElement
  root.style.setProperty('--accent', hex)
  root.style.setProperty('--on-accent', prefersDarkText(hex) ? '#000000' : '#ffffff')
}
