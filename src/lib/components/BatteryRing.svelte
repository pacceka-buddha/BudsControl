<script lang="ts">
  // The earbuds glyph inside a ring that fills with the earbuds' battery level.
  import EarbudsGlyph from './EarbudsGlyph.svelte'
  import { LOW_BATTERY } from '../format'

  interface Props {
    level: number | null
    charging?: boolean
    size?: number
  }
  let { level, charging = false, size = 34 }: Props = $props()

  const R = 46
  const C = 2 * Math.PI * R
  const tone = $derived(level == null ? 'none' : charging ? 'charging' : level <= LOW_BATTERY ? 'low' : 'normal')
  const offset = $derived(C * (1 - Math.min(Math.max(level ?? 0, 0), 100) / 100))
</script>

<div
  class="ring"
  style:width="{size}px"
  style:height="{size}px"
  role="img"
  aria-label={level == null ? 'Earbuds battery unavailable' : `Earbuds battery ${level} percent${charging ? ', charging' : ''}`}
>
  <svg viewBox="0 0 100 100" width={size} height={size}>
    <circle cx="50" cy="50" r={R} class="track" />
    {#if level != null}
      <circle
        cx="50"
        cy="50"
        r={R}
        class="arc {tone}"
        stroke-dasharray={C}
        stroke-dashoffset={offset}
        transform="rotate(-90 50 50)"
      />
    {/if}
  </svg>
  <span class="glyph"><EarbudsGlyph size={size * 0.52} /></span>
</div>

<style>
  .ring {
    position: relative;
    flex: none;
  }
  svg {
    display: block;
  }
  circle {
    fill: none;
    stroke-width: 8;
  }
  .track {
    stroke: var(--track);
    stroke-width: 8;
  }
  .arc {
    stroke-linecap: round;
    stroke: var(--accent);
    transition: stroke-dashoffset 500ms var(--ease-out), stroke 200ms;
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
