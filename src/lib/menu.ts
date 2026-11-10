// Drop-down menus. Inside the app they are native Windows menus (right look, right behaviour); in a
// plain browser (design review) a small in-page list stands in. While a native menu is open the
// flyout is told not to hide when it loses focus to the menu.

import { api } from './api'
import { isTauri } from './tauri'

export interface MenuEntry {
  id: string
  text: string
  /** Shows a check mark (the current choice). */
  checked?: boolean
  disabled?: boolean
}

export type MenuItems = (MenuEntry | 'separator')[]

/** Shows a menu under `anchor` and resolves to the chosen entry's id, or null if dismissed. */
export async function popupMenu(items: MenuItems, anchor: HTMLElement): Promise<string | null> {
  return isTauri ? nativeMenu(items) : pageMenu(items, anchor)
}

async function nativeMenu(items: MenuItems): Promise<string | null> {
  const { Menu, CheckMenuItem, MenuItem, PredefinedMenuItem } = await import('@tauri-apps/api/menu')
  await api.setMenuOpen(true)
  try {
    return await new Promise<string | null>((resolve) => {
      void (async () => {
        const built = await Promise.all(
          items.map((item) => {
            if (item === 'separator') return PredefinedMenuItem.new({ item: 'Separator' })
            const action = () => resolve(item.id)
            return item.checked !== undefined
              ? CheckMenuItem.new({ id: item.id, text: item.text, checked: item.checked, enabled: !item.disabled, action })
              : MenuItem.new({ id: item.id, text: item.text, enabled: !item.disabled, action })
          }),
        )
        const menu = await Menu.new({ items: built })
        await menu.popup()
        // The click's action arrives just after popup() returns; if none came, it was dismissed.
        setTimeout(() => resolve(null), 200)
      })()
    })
  } finally {
    // The menu took the focus; give it back, or close the flyout if the user clicked away.
    setTimeout(async () => {
      await api.setMenuOpen(false)
      if (!document.hasFocus()) void api.flyoutHide()
    }, 250)
  }
}

function pageMenu(items: MenuItems, anchor: HTMLElement): Promise<string | null> {
  return new Promise((resolve) => {
    const rect = anchor.getBoundingClientRect()
    const list = document.createElement('div')
    list.className = 'page-menu'
    list.setAttribute('role', 'menu')
    for (const item of items) {
      if (item === 'separator') {
        const hr = document.createElement('div')
        hr.className = 'page-menu-sep'
        list.append(hr)
        continue
      }
      const button = document.createElement('button')
      button.setAttribute('role', 'menuitem')
      button.disabled = !!item.disabled
      button.textContent = (item.checked ? '✓ ' : item.checked === false ? '   ' : '') + item.text
      button.onclick = () => close(item.id)
      list.append(button)
    }
    list.style.top = `${rect.bottom + 4}px`
    list.style.right = `${Math.max(4, window.innerWidth - rect.right)}px`
    document.body.append(list)
    const away = (e: Event) => {
      if (!list.contains(e.target as Node)) close(null)
    }
    const key = (e: KeyboardEvent) => e.key === 'Escape' && close(null)
    setTimeout(() => document.addEventListener('pointerdown', away), 0)
    document.addEventListener('keydown', key)
    function close(id: string | null) {
      document.removeEventListener('pointerdown', away)
      document.removeEventListener('keydown', key)
      list.remove()
      resolve(id)
    }
  })
}
