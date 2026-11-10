<script lang="ts">
  // The popup shown when a shortcut changes the mode. The window is transparent and click-through; this
  // page fades a pill in (150 ms), holds it (1.4 s) and fades it out (300 ms), then tells Rust to hide
  // the window. A new press while it is showing restarts the hold.
  import { onMount } from 'svelte'
  import type { HudPayload } from '../lib/bindings/HudPayload'
  import { api } from '../lib/api'
  import Icon from '../lib/components/Icon.svelte'
  import { LEVEL_TITLE, MODE_TITLE } from '../lib/format'
  import { listen } from '../lib/tauri'
  import { applyAccent } from '../lib/theme'

  const HOLD_MS = 1400
  const FADE_OUT_MS = 300

  let payload = $state<HudPayload | null>(null)
  let shown = $state(false)
  let lastId = 0
  let hold: ReturnType<typeof setTimeout> | undefined
  let done: ReturnType<typeof setTimeout> | undefined

  function present(next: HudPayload) {
    if (next.id <= lastId) return // already displayed (the event and the pending copy can both arrive)
    lastId = next.id
    payload = next
    shown = true
    clearTimeout(hold)
    clearTimeout(done)
    hold = setTimeout(() => {
      shown = false
      done = setTimeout(() => void api.hudDone(), FADE_OUT_MS)
    }, HOLD_MS)
  }

  const title = $derived(payload?.kind === 'mode' && payload.mode ? MODE_TITLE[payload.mode] : 'Buds not connected')
  const subtitle = $derived(
    payload?.kind === 'mode' && payload.level ? `${LEVEL_TITLE[payload.level]} level` : (payload?.deviceName ?? ''),
  )
  const icon = $derived(payload?.kind === 'mode' && payload.mode ? payload.mode : 'off')
  const active = $derived(payload?.kind === 'mode' && payload.mode !== 'off')

  onMount(() => {
    void applyAccent()
    const unlisten = listen<HudPayload>('hud://show', present)
    // The page may finish loading after the first event was emitted.
    void api.getHudPending().then((pending) => pending && present(pending))
    // Design review: hud.html?mode=anc&level=max shows it without the app.
    const params = new URLSearchParams(location.search)
    if (params.get('mode')) {
      present({
        id: 1,
        kind: 'mode',
        mode: params.get('mode') as HudPayload['mode'],
        level: (params.get('level') as HudPayload['level']) ?? null,
        deviceName: 'OnePlus Buds 4',
      })
    }
    return () => {
      clearTimeout(hold)
      clearTimeout(done)
      void unlisten.then((off) => off())
    }
  })
</script>

<div class="stage">
  <div class="pill" class:shown role="status" aria-live="polite">
    <span class="badge" class:active><Icon name={icon} size={24} /></span>
    <div class="text">
      <div class="title">{title}</div>
      <div class="sub">{subtitle}</div>
    </div>
  </div>
</div>

<style>
  :global(html) {
    background: transparent;
  }
  .stage {
    display: grid;
    place-items: center;
    height: 100vh;
    padding: 20px; /* room for the shadow */
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 28px 12px 12px;
    background: var(--surface-solid);
    border: 1px solid var(--stroke-strong);
    border-radius: var(--radius-pill);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.35);
    opacity: 0;
    transform: translateY(6px) scale(0.98);
    transition:
      opacity 300ms var(--ease-out),
      transform 300ms var(--ease-out);
  }
  .pill.shown {
    opacity: 1;
    transform: none;
    transition-duration: 150ms;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--fill-hover);
    color: var(--text);
  }
  .badge.active {
    background: var(--accent);
    color: var(--on-accent);
  }
  .title {
    font-size: 14px;
    font-weight: 600;
  }
  .sub {
    font-size: 12px;
    color: var(--text-2);
  }
</style>
