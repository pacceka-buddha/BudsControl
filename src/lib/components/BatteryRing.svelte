<script lang="ts">
  // The earbuds glyph inside a ring split in two, like the tray icon: the left half fills up from the
  // bottom with the left bud's battery, the right half with the right bud's.
  import type { Battery } from '../bindings/Battery'
  import EarbudsGlyph from './EarbudsGlyph.svelte'
  import { LOW_BATTERY } from '../format'

  interface Props {
    left: Battery | null
    right: Battery | null
    size?: number
  }
  let { left, right, size = 34 }: Props = $props()

  const R = 45
  const STROKE = 8
  // Each half stops short of 12 and 6 o'clock by half the gap plus the reach of its round cap.
  const HALF_GAP = (((7 + STROKE) / 2 / R) * 180) / Math.PI
  const SPAN = 180 - 2 * HALF_GAP

  /** An arc from `from` to `to` degrees, clockwise from 12 o'clock. */
  function arc(from: number, to: number): string {
    const point = (deg: number) => {
      const a = ((deg - 90) * Math.PI) / 180
      return `${(50 + R * Math.cos(a)).toFixed(2)} ${(50 + R * Math.sin(a)).toFixed(2)}`
    }
    return `M${point(from)} A${R} ${R} 0 0 1 ${point(to)}`
  }

  const tone = (b: Battery | null) => (b == null ? 'none' : b.charging ? 'charging' : b.level <= LOW_BATTERY ? 'low' : 'normal')
  const sweep = (b: Battery | null) => (SPAN * Math.min(Math.max(b?.level ?? 0, 0), 100)) / 100

  const rightEnd = HALF_GAP + SPAN
  const leftStart = 180 + HALF_GAP
  const label = $derived(
    left == null && right == null
      ? 'Earbuds battery unavailable'
      : `Left earbud ${left ? `${left.level} percent` : 'unknown'}, right earbud ${right ? `${right.level} percent` : 'unknown'}`,
  )
</script>

<div class="ring" style:width="{size}px" style:height="{size}px" role="img" aria-label={label}>
  <svg viewBox="0 0 100 100" width={size} height={size}>
    <path d={arc(HALF_GAP, rightEnd)} class="track" />
    <path d={arc(leftStart, leftStart + SPAN)} class="track" />
    {#if right && right.level > 0}
      <path d={arc(rightEnd - sweep(right), rightEnd)} class="arc {tone(right)}" />
    {/if}
    {#if left && left.level > 0}
      <path d={arc(leftStart, leftStart + sweep(left))} class="arc {tone(left)}" />
    {/if}
  </svg>
  <span class="glyph"><EarbudsGlyph size={size * 0.54} /></span>
</div>

<style>
  .ring {
    position: relative;
    flex: none;
  }
  svg {
    display: block;
  }
  path {
    fill: none;
    stroke-width: 8;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--track);
  }
  .arc {
    stroke: var(--accent);
    transition: stroke 200ms;
  }
  .arc.low {
    stroke: var(--danger);
  }
  .arc.charging {
    stroke: var(--success);
  }
  .glyph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text);
  }
</style>
