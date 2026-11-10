<script lang="ts">
  // Placeholder until global shortcuts arrive (Phase 8): lists the actions and any saved combination.
  import type { Settings } from '../lib/bindings/Settings'
  import Icon, { type IconName } from '../lib/components/Icon.svelte'
  import SettingRow from '../lib/components/SettingRow.svelte'

  interface Props {
    settings: Settings
  }
  let { settings }: Props = $props()

  const actions: { id: string; title: string; icon: IconName }[] = [
    { id: 'cycle', title: 'Cycle modes', icon: 'sync' },
    { id: 'anc', title: 'Noise Cancellation', icon: 'anc' },
    { id: 'transparency', title: 'Transparency', icon: 'transparency' },
    { id: 'off', title: 'Off', icon: 'off' },
  ]
</script>

<div class="page">
  <div class="group">
    {#each actions as action (action.id)}
      <SettingRow title={action.title}>
        <span class="icon"><Icon name={action.icon} size={16} /></span>
        <span class="combo" class:empty={!settings.shortcuts[action.id]}>{settings.shortcuts[action.id] ?? 'Not set'}</span>
      </SettingRow>
    {/each}
  </div>
  <p class="note">Global keyboard shortcuts (work in any app) can be recorded here in the next update.</p>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .icon {
    color: var(--text-2);
    display: grid;
  }
  .combo {
    min-width: 110px;
    padding: 4px 12px;
    text-align: center;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-control);
    background: var(--fill);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .combo.empty {
    color: var(--text-3);
    font-family: var(--font);
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-2);
  }
</style>
