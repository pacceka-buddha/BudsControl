<script lang="ts">
  // Global keyboard shortcuts: cycle the modes, or jump straight to one, from any app.
  import { api } from '../lib/api'
  import type { Settings } from '../lib/bindings/Settings'
  import Icon, { type IconName } from '../lib/components/Icon.svelte'
  import SettingRow from '../lib/components/SettingRow.svelte'
  import ShortcutRecorder from '../lib/components/ShortcutRecorder.svelte'
  import { app } from '../lib/stores.svelte'

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

  function set(action: string, accelerator: string | null) {
    const next = { ...settings.shortcuts }
    if (accelerator) next[action] = accelerator
    else delete next[action]
    void api.updateSettings({ shortcuts: next })
  }
</script>

<div class="page">
  <div class="group">
    {#each actions as action (action.id)}
      {@const conflict = app.conflicts.includes(action.id)}
      <SettingRow title={action.title} description={conflict ? 'Used by another app — pick a different combination.' : undefined}>
        <span class="icon"><Icon name={action.icon} size={16} /></span>
        <ShortcutRecorder
          label={action.title}
          value={settings.shortcuts[action.id] ?? null}
          {conflict}
          onchange={(accelerator) => set(action.id, accelerator)}
        />
      </SettingRow>
    {/each}
  </div>
  <p class="note">
    Shortcuts work everywhere, even when Buds Control isn't in front. Include Ctrl, Alt, Shift or Win (or use an F-key).
  </p>
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
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-2);
  }
  /* The conflict description is an error, not a hint. */
  .group :global(.desc) {
    color: var(--danger);
  }
</style>
