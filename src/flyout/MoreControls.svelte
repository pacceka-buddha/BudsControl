<script lang="ts">
  // The expandable part: equalizer, low latency, find my earbuds, gestures. Each card only appears
  // for what this model supports.
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import Button from '../lib/components/Button.svelte'
  import Card from '../lib/components/Card.svelte'
  import EqBands from '../lib/components/EqBands.svelte'
  import PickerButton from '../lib/components/PickerButton.svelte'
  import Row from '../lib/components/Row.svelte'
  import Segmented from '../lib/components/Segmented.svelte'
  import ToggleSwitch from '../lib/components/ToggleSwitch.svelte'
  import { namesFitAsButtons } from '../lib/format'
  import type { EarSide } from '../lib/bindings/EarSide'
  import type { Gesture } from '../lib/bindings/Gesture'
  import type { GestureAction } from '../lib/bindings/GestureAction'
  import type { Settings } from '../lib/bindings/Settings'
  import type { Snapshot } from '../lib/bindings/Snapshot'

  interface Props {
    snapshot: Snapshot
    settings: Settings | null
  }
  let { snapshot, settings }: Props = $props()

  // --- Equalizer: buttons when every preset name fits on one row, otherwise a menu ---
  const FONT = '600 11px "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif'
  const ROW_WIDTH = 250 // 300 window - 24 padding - 20 card - 6 track
  let measure: CanvasRenderingContext2D | null = null
  onMount(() => {
    measure = document.createElement('canvas').getContext('2d')
    if (measure) measure.font = FONT
  })
  function width(text: string): number {
    return measure ? measure.measureText(text).width : text.length * 7
  }
  const eqItems = $derived(snapshot.eqChoices.map((c) => ({ id: String(c.id), label: c.name })))
  const eqAsButtons = $derived(namesFitAsButtons(snapshot.eqChoices.map((c) => width(c.name)), ROW_WIDTH))
  const eqCurrent = $derived(snapshot.eqChoices.find((c) => c.id === snapshot.eqId))

  // --- Gestures ---
  const sides = $derived(
    (['left', 'right'] as EarSide[]).filter((s) => snapshot.gestures[s] != null).map((s) => ({ id: s, label: s === 'left' ? 'Left' : 'Right' })),
  )
  const side: EarSide = $derived(sides.find((s) => s.id === settings?.gestureSide)?.id ?? sides[0]?.id ?? 'left')
  const GESTURES: { id: Gesture; title: string }[] = [
    { id: 'doubleTap', title: 'Double tap' },
    { id: 'tripleTap', title: 'Triple tap' },
    { id: 'hold', title: 'Touch & hold' },
  ]
  const gestureRows = $derived(
    GESTURES.flatMap((g) => {
      const slot = snapshot.gestures[side]?.[g.id]
      return slot ? [{ ...g, slot }] : []
    }),
  )
</script>

<div class="more" id="more-controls">
  {#if snapshot.supports.equalizer}
    <Card title="Equalizer" icon="equalizer">
      {#if eqAsButtons}
        <Segmented
          label="Equalizer preset"
          items={eqItems}
          selected={snapshot.eqId == null ? null : String(snapshot.eqId)}
          variant="even"
          fit
          height={22}
          onselect={(id) => api.setEq(Number(id))}
        >
          {#snippet segment(item)}<span class="seg-text">{item.label}</span>{/snippet}
        </Segmented>
      {:else}
        <PickerButton
          label="Preset"
          value={eqCurrent?.name ?? '—'}
          current={snapshot.eqId == null ? null : String(snapshot.eqId)}
          options={eqItems.map((i) => ({ id: i.id, text: i.label }))}
          onselect={(id) => api.setEq(Number(id))}
        />
      {/if}
      {#if snapshot.activeCustomEq}
        <EqBands eq={snapshot.activeCustomEq} oncommit={(gains) => api.setCustomEq(gains)} />
      {/if}
    </Card>
  {/if}

  {#if snapshot.supports.lowLatency}
    <Card>
      <Row icon="gamepad" title="Low latency" subtitle="For games and video">
        {#snippet accessory()}
          <ToggleSwitch label="Low latency" checked={snapshot.lowLatency ?? false} onchange={(on) => api.setLowLatency(on)} />
        {/snippet}
      </Row>
    </Card>
  {/if}

  {#if snapshot.supports.findBuds}
    <Card>
      <Row
        icon="find"
        title="Find earbuds"
        subtitle={snapshot.findingBuds ? 'Playing a sound…' : 'Loud — take them out of your ears first'}
      >
        {#snippet accessory()}
          <Button variant={snapshot.findingBuds ? 'danger' : 'default'} onclick={() => api.findBuds(!snapshot.findingBuds)}>
            {snapshot.findingBuds ? 'Stop' : 'Play'}
          </Button>
        {/snippet}
      </Row>
    </Card>
  {/if}

  {#if snapshot.supports.gestures && sides.length > 0}
    <Card title="Gestures" icon="gestures">
      <Segmented
        label="Earbud"
        items={sides}
        selected={side}
        variant="even"
        height={22}
        onselect={(id) => api.updateSettings({ gestureSide: id as EarSide })}
      >
        {#snippet segment(item)}<span class="seg-text">{item.label}</span>{/snippet}
      </Segmented>
      {#each gestureRows as row (row.id)}
        <PickerButton
          label={row.title}
          value={row.slot.options.find((o) => o.action === row.slot.current)?.title ?? '—'}
          current={row.slot.current}
          options={row.slot.options.map((o) => ({ id: o.action, text: o.title }))}
          onselect={(action) => api.setGesture(side, row.id, action as GestureAction)}
        />
      {/each}
    </Card>
  {/if}
</div>

<style>
  .more {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 10px;
  }
  .seg-text {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
