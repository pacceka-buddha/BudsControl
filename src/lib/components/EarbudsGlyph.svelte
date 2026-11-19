<script lang="ts">
  // Our own earbuds drawing (same geometry as the app and tray icons): two upright buds, or one bud /
  // the case.
  interface Props {
    size?: number
    part?: 'both' | 'left' | 'right' | 'case'
  }
  let { size = 20, part = 'both' }: Props = $props()
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" focusable="false">
  {#snippet bud(x: number)}
    <ellipse cx={x} cy="8.9" rx="3" ry="3.7" />
    <rect x={x - 1.3} y="10" width="2.6" height="9.3" rx="1.3" />
  {/snippet}
  {#if part === 'case'}
    <rect x="4" y="6" width="16" height="13" rx="5" fill="none" stroke="currentColor" stroke-width="1.8" />
    <path d="M4.5 11.5h15" fill="none" stroke="currentColor" stroke-width="1.8" />
  {:else if part === 'left' || part === 'right'}
    {@render bud(12)}
  {:else}
    {@render bud(7)}
    {@render bud(17)}
  {/if}
</svg>
