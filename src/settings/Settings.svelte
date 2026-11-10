<script lang="ts">
  // The settings window: General, Shortcuts, and (when developer options are on) Developer.
  import { onMount } from 'svelte'
  import Tabs from '../lib/components/Tabs.svelte'
  import { app, initStores } from '../lib/stores.svelte'
  import { applyAccent } from '../lib/theme'
  import DeveloperTab from './DeveloperTab.svelte'
  import GeneralTab from './GeneralTab.svelte'
  import ShortcutsTab from './ShortcutsTab.svelte'

  type TabId = 'general' | 'shortcuts' | 'developer'

  const initial = new URLSearchParams(location.search).get('tab')
  let tab: TabId = $state(initial === 'developer' || initial === 'shortcuts' ? initial : 'general')

  const developer = $derived(app.settings?.developerMode ?? false)
  const tabs = $derived([
    { id: 'general', label: 'General' },
    { id: 'shortcuts', label: 'Shortcuts' },
    ...(developer ? [{ id: 'developer', label: 'Developer' }] : []),
  ])

  // Turning developer options off while on that tab returns to General.
  $effect(() => {
    if (!developer && tab === 'developer' && app.settings) tab = 'general'
  })

  onMount(() => {
    void initStores()
    void applyAccent()
  })
</script>

<main>
  <h1>Settings</h1>
  <Tabs {tabs} selected={tab} label="Settings sections" onselect={(id) => (tab = id as TabId)} />
  <div class="panel" role="tabpanel" id="panel-{tab}" aria-labelledby="tab-{tab}">
    {#if !app.snapshot || !app.settings}
      <p class="loading">Loading…</p>
    {:else if tab === 'general'}
      <GeneralTab snapshot={app.snapshot} settings={app.settings} />
    {:else if tab === 'shortcuts'}
      <ShortcutsTab settings={app.settings} />
    {:else}
      <DeveloperTab snapshot={app.snapshot} />
    {/if}
  </div>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    padding: 20px 24px 0;
  }
  h1 {
    margin: 0 0 12px;
    font-size: 20px;
    font-weight: 600;
  }
  .panel {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 4px 24px 0;
  }
  .loading {
    color: var(--text-2);
  }
</style>
