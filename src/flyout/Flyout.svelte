<script lang="ts">
  // The tray flyout: device header, noise controls, battery, and an expandable panel with the
  // equalizer, low latency, find my earbuds and gestures.
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { app, initStores } from '../lib/stores.svelte'
  import { listen } from '../lib/tauri'
  import { applyAccent } from '../lib/theme'
  import BatteryRow from './BatteryRow.svelte'
  import Disconnected from './Disconnected.svelte'
  import Header from './Header.svelte'
  import ModeControls from './ModeControls.svelte'
  import MoreControls from './MoreControls.svelte'

  /** Matches the CSS transition of the More panel (plus a little slack). */
  const ANIMATION_MS = 460

  let root: HTMLElement | undefined = $state()
  let canvas: HTMLCanvasElement | undefined = $state()
  let inner: HTMLElement | undefined = $state()
  // The More panel's own height, which the collapsible animates to.
  let moreHeight = $state(0)

  const snapshot = $derived(app.snapshot)
  const connected = $derived(snapshot?.link === 'connected')

  // Whether the user has the More panel open; starts from the saved setting.
  let moreOpen = $state(false)
  let moreRestored = false
  // Transitions stay off until the saved state is applied, so it appears instantly on load.
  let animate = $state(false)
  $effect(() => {
    if (app.settings && !moreRestored) {
      moreRestored = true
      moreOpen = app.settings.moreExpanded
      requestAnimationFrame(() => requestAnimationFrame(() => (animate = true)))
    }
  })
  const expanded = $derived(moreOpen && connected && (snapshot?.hasMoreControls ?? false))

  // The window follows the content height, except while the panel animates: opening grows the
  // window first (so the panel has room), closing shrinks it afterwards.
  let locked = false
  // Rounded up: `offsetHeight` rounds a fractional height down, and a window even a fraction of a
  // pixel short shows a scrollbar, which narrows the text so it wraps and overflows further.
  const contentHeight = (el: HTMLElement) => Math.ceil(el.getBoundingClientRect().height)
  function report() {
    if (!locked && root) void api.flyoutResize(contentHeight(root))
  }

  async function toggleMore() {
    if (!root || !inner) return
    const opening = !moreOpen
    locked = true
    if (opening) {
      await api.flyoutResize(contentHeight(root) + inner.scrollHeight)
      moreOpen = true
    } else {
      moreOpen = false
    }
    void api.updateSettings({ moreExpanded: opening })
    setTimeout(() => {
      locked = false
      report()
    }, ANIMATION_MS)
  }

  // Windows: a blurred snapshot of what is behind the flyout, taken by the app just before it opens
  // (see backdrop.rs), drawn under the translucent surface: frosted glass that doesn't depend on the
  // window effects Windows may or may not draw. Where it lies, relative to the window:
  interface BackdropInfo {
    generation: number
    width: number
    height: number
    left: number
    top: number
    bottom: number
    cssWidth: number
    cssHeight: number
    pinBottom: boolean
  }
  let backdrop: BackdropInfo | null = $state(null)
  const nextFrame = () =>
    new Promise<void>((done) => {
      // A hidden window may not get animation frames; don't wait long for one.
      requestAnimationFrame(() => done())
      setTimeout(done, 40)
    })

  /** `rgba(…)`, `rgb(…)` or `#rrggbb` as [r, g, b, alpha]. */
  function parseColor(text: string): [number, number, number, number] | null {
    const hex = text.trim().match(/^#([0-9a-f]{6})$/i)
    if (hex) {
      const n = parseInt(hex[1], 16)
      return [n >> 16, (n >> 8) & 255, n & 255, 1]
    }
    const m = text.match(/rgba?\(([^)]+)\)/)
    if (!m) return null
    const [r, g, b, a = 1] = m[1].split(',').map(Number)
    return [r, g, b, a]
  }

  // Windows 11 keeps a one-pixel strip of title bar above the page (see `set_flyout_edge`). Give it
  // the colour the glass has along the top edge: the snapshot there, saturated as the CSS does, under
  // the surface and the lit rim.
  let lastEdge = -1
  async function sendEdge() {
    const style = getComputedStyle(document.documentElement)
    let colour: number[] | null = null
    const context = canvas?.getContext('2d')
    const info = backdrop
    if (info && context) {
      const canvasTop = info.pinBottom ? innerHeight - info.bottom - info.cssHeight : info.top
      const sx = info.width / info.cssWidth
      const row = Math.min(info.height - 1, Math.max(0, Math.floor((-1 - canvasTop) * (info.height / info.cssHeight))))
      const x0 = Math.max(0, Math.floor(-info.left * sx))
      const x1 = Math.min(info.width, Math.ceil((innerWidth - info.left) * sx))
      const data = context.getImageData(x0, row, Math.max(1, x1 - x0), 1).data
      let [r, g, b] = [0, 0, 0]
      for (let i = 0; i < data.length; i += 4) {
        r += data[i]
        g += data[i + 1]
        b += data[i + 2]
      }
      const n = data.length / 4
      ;[r, g, b] = [r / n, g / n, b / n]
      const k = 1.8 // saturate(1.8)
      colour = [
        (0.213 + 0.787 * k) * r + (0.715 - 0.715 * k) * g + (0.072 - 0.072 * k) * b,
        (0.213 - 0.213 * k) * r + (0.715 + 0.285 * k) * g + (0.072 - 0.072 * k) * b,
        (0.213 - 0.213 * k) * r + (0.715 - 0.715 * k) * g + (0.072 + 0.928 * k) * b,
      ].map((v) => Math.min(255, Math.max(0, v)))
      for (const name of ['--surface', '--hi']) {
        const layer = parseColor(style.getPropertyValue(name))
        if (layer) colour = colour.map((v, i) => v * (1 - layer[3]) + layer[i] * layer[3])
      }
    } else {
      colour = parseColor(style.getPropertyValue('--surface-solid'))
    }
    if (!colour) return
    const [r, g, b] = colour.map(Math.round)
    const value = (b << 16) | (g << 8) | r
    if (value === lastEdge) return
    lastEdge = value
    await api.flyoutEdge(value)
  }

  async function drawBackdrop(info: BackdropInfo) {
    let drawn = false
    try {
      const pixels = new Uint8ClampedArray(await api.flyoutBackdrop())
      const context = canvas?.getContext('2d')
      if (canvas && context && info.width > 0 && pixels.length === info.width * info.height * 4) {
        canvas.width = info.width
        canvas.height = info.height
        context.putImageData(new ImageData(pixels, info.width, info.height), 0, 0)
        drawn = true
      }
    } catch {
      // drawn stays false: solid surface
    }
    backdrop = drawn ? info : null
    document.documentElement.classList.toggle('backdrop', drawn)
    document.documentElement.classList.toggle('solid', !drawn)
    await sendEdge().catch(() => {})
    await nextFrame()
    void api.flyoutReveal(info.generation)
  }

  onMount(() => {
    void initStores()
    void applyAccent()
    // Linux: the window is transparent and the page draws its own rounded sheet (see tokens.css).
    void api
      .getPlatform()
      .then((info) => document.documentElement.classList.toggle('backdrop', info.os === 'linux'))
      .catch(() => {})
    const unlisten = listen<null>('flyout-shown', () => void applyAccent())
    const unlistenBackdrop = listen<BackdropInfo>('flyout-backdrop', (info) => void drawBackdrop(info))

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') void api.flyoutHide()
    }
    window.addEventListener('keydown', onKey)
    // The window's top edge moves over the snapshot as it grows and shrinks.
    const onResize = () => void sendEdge().catch(() => {})
    window.addEventListener('resize', onResize)

    const observer = new ResizeObserver(report)
    if (root) observer.observe(root)

    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('resize', onResize)
      observer.disconnect()
      void unlisten.then((off) => off())
      void unlistenBackdrop.then((off) => off())
    }
  })
</script>

<div class="backdrop" aria-hidden="true">
  <canvas
    bind:this={canvas}
    style:left="{backdrop?.left ?? 0}px"
    style:top={backdrop && !backdrop.pinBottom ? `${backdrop.top}px` : null}
    style:bottom={backdrop?.pinBottom ? `${backdrop.bottom}px` : null}
    style:width="{backdrop?.cssWidth ?? 0}px"
    style:height="{backdrop?.cssHeight ?? 0}px"
  ></canvas>
</div>

<main bind:this={root}>
  {#if snapshot}
    <Header {snapshot} expanded={expanded} ontoggle={toggleMore} />
    {#if connected}
      {#if snapshot.supports.noiseControl}
        <ModeControls {snapshot} />
      {/if}
      <div class="stack">
        <BatteryRow {snapshot} />
        {#if snapshot.hasMoreControls}
          <div
            class="collapsible"
            class:open={expanded}
            class:animate
            inert={!expanded}
            style:height={expanded ? `${moreHeight}px` : '0px'}
          >
            <div class="inner" bind:this={inner} bind:offsetHeight={moreHeight}>
              <MoreControls {snapshot} settings={app.settings} />
            </div>
          </div>
        {/if}
      </div>
    {:else}
      <Disconnected {snapshot} />
    {/if}
  {/if}
</main>

<style>
  /* Fixed to the window, under everything; the canvas sits where the snapshot was taken, pinned to the
     edge the window grows away from, so resizing the window doesn't move it. */
  .backdrop {
    display: none;
    position: fixed;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    pointer-events: none;
    /* Windows 11's corner radius for small windows (Linux sets its own). */
    border-radius: var(--sheet-radius, 8px);
  }
  :global(:root.backdrop) .backdrop {
    display: block;
  }
  canvas {
    position: absolute;
    filter: saturate(1.8);
  }
  /* The translucent surface over the snapshot, with a faint edge and a lit top edge. */
  .backdrop::after {
    content: '';
    position: absolute;
    inset: 0;
    background: var(--sheet, var(--surface));
    border-radius: inherit;
    box-shadow:
      inset 0 1px 0 var(--hi),
      inset 0 0 0 1px var(--stroke);
  }
  main {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    padding: var(--pad);
  }
  /* Opens and closes like a disclosure: the height goes from 0 to the content's measured height. (Not a
     0fr -> 1fr grid row: WebKit kept such a row at its old size when the content shrank, so the window
     stayed taller than the flyout.) */
  .collapsible {
    overflow: hidden;
    opacity: 0;
  }
  .collapsible.animate {
    transition:
      height 420ms cubic-bezier(0.32, 0.9, 0.4, 1),
      opacity 300ms var(--ease-out);
  }
  .collapsible.open {
    opacity: 1;
  }
</style>
