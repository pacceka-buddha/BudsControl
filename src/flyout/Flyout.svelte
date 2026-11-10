<script lang="ts">
  // PHASE 4 DEBUG VIEW: shows the live controller snapshot as JSON and the quick actions, to check
  // the Rust <-> web bridge. Phase 5 replaces this with the real flyout.
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { app, initStores } from '../lib/stores.svelte'

  let root: HTMLElement | undefined = $state()

  onMount(() => {
    void initStores()

    // Esc closes the flyout.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') void api.flyoutHide()
    }
    window.addEventListener('keydown', onKey)

    // The window follows the content height.
    const observer = new ResizeObserver(() => {
      if (root) void api.flyoutResize(root.offsetHeight)
    })
    if (root) observer.observe(root)

    return () => {
      window.removeEventListener('keydown', onKey)
      observer.disconnect()
    }
  })
</script>

<main bind:this={root}>
  <header>
    <strong>{app.snapshot?.deviceName ?? 'Buds Control'}</strong>
    <span class="sub">{app.snapshot ? app.snapshot.link : 'loading…'}{app.fake ? ' · simulated' : ''}</span>
  </header>
  <div class="row">
    <button onclick={() => api.setMode('off')}>Off</button>
    <button onclick={() => api.setMode('transparency')}>Transparency</button>
    <button onclick={() => api.setMode('anc')}>ANC</button>
  </div>
  <div class="row">
    <button onclick={() => api.reconnect()}>Reconnect</button>
    <button onclick={() => api.openSettings()}>Settings</button>
    <button onclick={() => api.quit()}>Quit</button>
  </div>
  <pre>{JSON.stringify(app.snapshot, null, 1)}</pre>
</main>

<style>
  main {
    padding: 12px;
    background: Canvas;
    color: CanvasText;
    border-radius: 8px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 8px;
  }
  .sub {
    font-size: 12px;
    opacity: 0.7;
  }
  .row {
    display: flex;
    gap: 6px;
    margin-bottom: 6px;
  }
  button {
    flex: 1;
    padding: 4px 6px;
    font: inherit;
    font-size: 12px;
  }
  pre {
    margin: 8px 0 0;
    max-height: 300px;
    overflow: auto;
    font-size: 10px;
    user-select: text;
    background: rgba(128, 128, 128, 0.12);
    padding: 6px;
    border-radius: 4px;
  }
</style>
