<script lang="ts">
  // Off / Transparency / Noise Cancellation, plus the ANC strength levels when ANC is on.
  import { api } from '../lib/api'
  import Icon from '../lib/components/Icon.svelte'
  import Segmented from '../lib/components/Segmented.svelte'
  import { LEVEL_TITLE, MODE_SHORT, MODE_TITLE } from '../lib/format'
  import type { AncLevel } from '../lib/bindings/AncLevel'
  import type { NoiseMode } from '../lib/bindings/NoiseMode'
  import type { Snapshot } from '../lib/bindings/Snapshot'

  interface Props {
    snapshot: Snapshot
  }
  let { snapshot }: Props = $props()

  const modes = $derived(snapshot.availableModes.map((m) => ({ id: m, label: MODE_TITLE[m] })))
  const levels = $derived(snapshot.availableLevels.map((l) => ({ id: l, label: LEVEL_TITLE[l] })))
  const showLevels = $derived(snapshot.mode === 'anc' && levels.length > 0)
</script>

<Segmented
  label="Noise control"
  items={modes}
  selected={snapshot.mode}
  variant="expand"
  height={32}
  onselect={(id) => api.setMode(id as NoiseMode)}
>
  {#snippet segment(item, on)}
    <Icon name={item.id as NoiseMode} size={16} />
    {#if on}<span class="text">{MODE_SHORT[item.id as NoiseMode]}</span>{/if}
  {/snippet}
</Segmented>

{#if showLevels}
  <div class="levels">
    <Segmented
      label="Noise cancellation level"
      items={levels}
      selected={snapshot.level}
      variant="even"
      fit
      height={22}
      onselect={(id) => api.setMode('anc', id as AncLevel)}
    >
      {#snippet segment(item)}
        <span class="level">{item.label}</span>
      {/snippet}
    </Segmented>
  </div>
{/if}

<style>
  .text {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .level {
    font-size: 11px;
  }
  .levels {
    animation: appear var(--dur) var(--ease-out);
  }
  @keyframes appear {
    from {
      opacity: 0;
    }
  }
</style>
