<script lang="ts">
  // Off / Transparency / Noise Cancellation, plus the ANC strength levels when ANC is on.
  import { blur } from 'svelte/transition'
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
    <Icon name={item.id as NoiseMode} size={15} />
    <span class="text" class:shown={on} aria-hidden={!on}>{MODE_SHORT[item.id as NoiseMode]}</span>
  {/snippet}
</Segmented>

{#if showLevels}
  <div class="levels" transition:blur={{ duration: 260, amount: 5 }}>
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
  /* Unselected modes are icon-only: the label grows in from nothing as its segment widens. */
  .text {
    max-width: 0;
    opacity: 0;
    transform: scale(0.6);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    transition:
      max-width 420ms var(--ease-spring),
      opacity 250ms,
      transform 420ms var(--ease-spring);
  }
  .text.shown {
    max-width: 110px;
    opacity: 1;
    transform: none;
  }
  .level {
    font-size: 11px;
  }
  .levels {
    margin-top: -2px;
  }
</style>
