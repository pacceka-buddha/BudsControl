<script lang="ts">
  // Band sliders for the active custom EQ. Edits show at once (a local draft); the curve is sent when
  // a drag ends, or shortly after the last key press.
  import type { CustomEq } from '../bindings/CustomEq'
  import { gainLabel, hzLabel } from '../format'
  import BandSlider from './BandSlider.svelte'

  interface Props {
    eq: CustomEq
    oncommit: (gains: number[]) => void
  }
  let { eq, oncommit }: Props = $props()

  let draft: number[] | null = $state(null)
  /** True from the first edit until a moment after the curve was sent. */
  let editing = false
  let sendTimer: ReturnType<typeof setTimeout> | undefined
  let settleTimer: ReturnType<typeof setTimeout> | undefined

  const gains = $derived(draft ?? eq.bands.map((b) => b.gain))

  // A reading from the buds replaces the draft, unless an edit is still in progress.
  $effect(() => {
    void eq
    if (!editing) draft = null
  })

  function change(index: number, value: number) {
    editing = true
    clearTimeout(settleTimer)
    const next = [...gains]
    next[index] = value
    draft = next
  }

  function commit(delay: number) {
    clearTimeout(sendTimer)
    sendTimer = setTimeout(() => {
      if (draft) oncommit(draft)
      // Keep the draft on screen until the buds' answer has had time to arrive.
      settleTimer = setTimeout(() => {
        editing = false
        draft = null
      }, 800)
    }, delay)
  }
</script>

<div class="bands" role="group" aria-label="Custom equalizer bands">
  {#each eq.bands as band, i (band.hz)}
    <div class="band">
      <span class="gain" class:zero={gains[i] === 0}>{gainLabel(gains[i])}</span>
      <BandSlider
        value={gains[i]}
        min={eq.minGain}
        max={eq.maxGain}
        label="{hzLabel(band.hz)} hertz"
        valueText="{gains[i]} decibels"
        onchange={(v) => change(i, v)}
        oncommit={() => commit(120)}
      />
      <span class="hz">{hzLabel(band.hz)}</span>
    </div>
  {/each}
</div>

<style>
  .bands {
    display: flex;
    gap: 4px;
  }
  .band {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: 0;
  }
  .gain {
    font-size: 10px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .gain.zero {
    color: var(--text-2);
  }
  .hz {
    font-size: 10px;
    color: var(--text-2);
  }
</style>
