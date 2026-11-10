// Applies the user's Windows accent colour to the page's design tokens.

import { api } from './api'
import { prefersDarkText } from './format'

/** Marks the page with the OS (`<html data-platform="linux">`), so the CSS can adapt. */
export async function applyPlatform(): Promise<void> {
  const info = await api.getPlatform().catch(() => null)
  if (info) document.documentElement.dataset.platform = info.os
}

export async function applyAccent(): Promise<void> {
  void applyPlatform()
  const hex = await api.getAccentColor().catch(() => null)
  if (!hex) return
  const root = document.documentElement
  root.style.setProperty('--accent', hex)
  root.style.setProperty('--on-accent', prefersDarkText(hex) ? '#000000' : '#ffffff')
}
