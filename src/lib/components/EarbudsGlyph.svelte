<script lang="ts">
  // Our own earbuds drawing (same geometry as the app icon): two buds, or one bud / the case.
  interface Props {
    size?: number
    part?: 'both' | 'left' | 'right' | 'case'
  }
  let { size = 20, part = 'both' }: Props = $props()

  // One bud, drawn for the left ear around its head centre; `mirror` flips it for the right ear.
  const HEAD_RX = 3.2
  const HEAD_RY = 3.6
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
  {#snippet bud(x: number, mirror: boolean)}
    <g transform={`translate(${x} 6.6) ${mirror ? 'scale(-1 1)' : ''} rotate(14)`}>
      <rect x="-0.4" y="0" width="2.2" height="11.2" rx="1.1" />
      <ellipse cx="0" cy="0" rx={HEAD_RX} ry={HEAD_RY} />
    </g>
  {/snippet}
  {#if part === 'case'}
    <rect x="4" y="6" width="16" height="13" rx="5" fill="none" stroke="currentColor" stroke-width="1.8" />
    <path d="M4.5 11.5h15" fill="none" stroke="currentColor" stroke-width="1.8" />
  {:else if part === 'left'}
    {@render bud(12, false)}
  {:else if part === 'right'}
    {@render bud(12, true)}
  {:else}
    {@render bud(7.6, false)}
    {@render bud(16.4, true)}
  {/if}
</svg>
