<script lang="ts">
  // Global keyboard shortcuts: cycle the modes, or jump straight to one, from any app.
  import { api } from '../lib/api'
  import type { Settings } from '../lib/bindings/Settings'
  import Icon, { type IconName } from '../lib/components/Icon.svelte'
  import SettingRow from '../lib/components/SettingRow.svelte'
  import ShortcutRecorder from '../lib/components/ShortcutRecorder.svelte'
  import { app } from '../lib/stores.svelte'
  import { onMount } from 'svelte'

  // Wayland has no global shortcuts for ordinary apps; the desktop's own keyboard settings do the job.
  let wayland = $state(false)
  onMount(() => {
    void api.getPlatform().then((info) => (wayland = info.wayland))
  })

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
  {#if wayland}
    <p class="note warn">
      This desktop session (Wayland) doesn't allow apps to register global shortcuts. In your desktop's keyboard
      settings, bind a key to a command instead: <code>budsctl cycle</code>, <code>budsctl anc</code>,
      <code>budsctl transparency</code> or <code>budsctl off</code>.
    </p>
  {:else}
    <p class="note">
      Shortcuts work everywhere, even when Buds Control isn't in front. Include Ctrl, Alt, Shift or Win (or use an
      F-key).
    </p>
  {/if}
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
  .warn {
    padding: 8px 10px;
    border-radius: var(--radius-control);
    background: var(--fill);
  }
  code {
    font-family: var(--font-mono);
  }
  /* The conflict description is an error, not a hint. */
  .group :global(.desc) {
    color: var(--danger);
  }
</style>
