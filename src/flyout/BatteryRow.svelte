<script lang="ts">
  // Left, right and case battery on one line.
  import type { Battery } from '../lib/bindings/Battery'
  import type { Snapshot } from '../lib/bindings/Snapshot'
  import EarbudsGlyph from '../lib/components/EarbudsGlyph.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import { batteryTone } from '../lib/format'

  interface Props {
    snapshot: Snapshot
  }
  let { snapshot }: Props = $props()

  const items: { part: 'left' | 'right' | 'case'; name: string; battery: Battery | null }[] = $derived([
    { part: 'left', name: 'Left earbud', battery: snapshot.left },
    { part: 'right', name: 'Right earbud', battery: snapshot.right },
    { part: 'case', name: 'Case', battery: snapshot.case },
  ])
</script>

<div class="row">
  {#each items as item (item.part)}
    {@const tone = batteryTone(item.battery)}
    <div
      class="item"
      role="img"
      aria-label={item.battery ? `${item.name} ${item.battery.level} percent${item.battery.charging ? ', charging' : ''}` : `${item.name} battery unknown`}
    >
      <span class="glyph"><EarbudsGlyph part={item.part} size={14} /></span>
      {#if item.battery?.charging}<span class="bolt"><Icon name="bolt" size={11} /></span>{/if}
      <span class="level {tone}">{item.battery ? `${item.battery.level}%` : '—'}</span>
    </div>
  {/each}
</div>

<style>
  .row {
    display: flex;
    justify-content: space-between;
    padding: 0 6px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .glyph {
    color: var(--text-2);
    display: grid;
  }
  .bolt {
    color: var(--success);
    display: grid;
  }
  .level {
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .level.low {
    color: var(--danger);
  }
</style>
