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
  const ANIMATION_MS = 320

  let root: HTMLElement | undefined = $state()
  let inner: HTMLElement | undefined = $state()

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
  function report() {
    if (!locked && root) void api.flyoutResize(root.offsetHeight)
  }

  async function toggleMore() {
    if (!root || !inner) return
    const opening = !moreOpen
    locked = true
    if (opening) {
      await api.flyoutResize(root.offsetHeight + inner.scrollHeight)
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

  onMount(() => {
    void initStores()
    void applyAccent()
    const unlisten = listen<null>('flyout-shown', () => void applyAccent())

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') void api.flyoutHide()
    }
    window.addEventListener('keydown', onKey)

    const observer = new ResizeObserver(report)
    if (root) observer.observe(root)

    return () => {
      window.removeEventListener('keydown', onKey)
      observer.disconnect()
      void unlisten.then((off) => off())
    }
  })
</script>

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
          <div class="collapsible" class:open={expanded} class:animate inert={!expanded}>
            <div class="inner" bind:this={inner}>
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
  main {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    padding: var(--pad);
  }
  /* Opens and closes like a disclosure: the row grows from 0 to the content's height. */
  .collapsible {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
  }
  .collapsible.animate {
    transition:
      grid-template-rows var(--dur) var(--ease-out),
      opacity var(--dur) var(--ease-out);
  }
  .collapsible.open {
    grid-template-rows: 1fr;
    opacity: 1;
  }
  .inner {
    min-height: 0;
    overflow: hidden;
  }
</style>
