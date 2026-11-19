<script lang="ts" module>
  // Simple 24x24 line icons, drawn for this app (stroke = currentColor).
  interface Def {
    d: string[]
    /** Stroke width; the dot icons need a fat one. */
    sw?: number
    fill?: boolean
  }

  const ICONS = {
    off: { d: ['M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17z', 'M6 6l12 12'] },
    // An ear with sound arriving: "transparency" lets the outside in.
    transparency: {
      d: [
        'M7 10a5 5 0 0 1 10 0c0 2.6-2 3.6-3 5.2-.8 1.3-.5 3.3-2.5 3.8-1.6.4-2.8-.6-3-2',
        'M10.5 10a1.5 1.5 0 0 1 3 0',
      ],
    },
    // Over-ear headphones: noise cancellation.
    anc: {
      d: ['M4 15v-3a8 8 0 0 1 16 0v3', 'M4 14h3.5v6H5.5A1.5 1.5 0 0 1 4 18.5z', 'M20 14h-3.5v6h2a1.5 1.5 0 0 0 1.5-1.5z'],
    },
    bolt: { d: ['M13.2 2.5 5.5 13h5.3l-1 8.5 8.7-11.5h-5.4z'], fill: true, sw: 1 },
    equalizer: {
      d: ['M4 7h16', 'M4 12h16', 'M4 17h16', 'M9 5a2 2 0 1 0 0 4 2 2 0 0 0 0-4z', 'M15 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4z', 'M8 15a2 2 0 1 0 0 4 2 2 0 0 0 0-4z'],
    },
    gamepad: {
      d: [
        'M7 8h10a4 4 0 0 1 4 4.2l-.6 3.4a2.5 2.5 0 0 1-4.3 1.2L14.5 15h-5l-1.6 1.8a2.5 2.5 0 0 1-4.3-1.2L3 12.2A4 4 0 0 1 7 8z',
        'M8 10.5v3',
        'M6.5 12h3',
        'M15.5 11.5h.01',
        'M17.5 13h.01',
      ],
    },
    find: {
      d: ['M12 12h.01', 'M8.6 8.6a4.8 4.8 0 0 0 0 6.8', 'M15.4 8.6a4.8 4.8 0 0 1 0 6.8', 'M5.7 5.7a8.9 8.9 0 0 0 0 12.6', 'M18.3 5.7a8.9 8.9 0 0 1 0 12.6'],
      sw: 1.6,
    },
    gestures: {
      d: [
        'M9.5 12V5.8a1.6 1.6 0 0 1 3.2 0V11',
        'M12.7 11l3.2.7a2 2 0 0 1 1.6 2.3l-.6 4.5a1.5 1.5 0 0 1-1.5 1.3h-4.2a1.5 1.5 0 0 1-1.2-.6L7 15.8a1.3 1.3 0 0 1 2-1.6l.5.5',
      ],
    },
    settings: {
      d: [
        'M12 14.8a2.8 2.8 0 1 0 0-5.6 2.8 2.8 0 0 0 0 5.6z',
        'M12 3.5v2.3', 'M12 18.2v2.3', 'M3.5 12h2.3', 'M18.2 12h2.3',
        'M6 6l1.6 1.6', 'M16.4 16.4 18 18', 'M6 18l1.6-1.6', 'M16.4 7.6 18 6',
      ],
    },
    more: { d: ['M6 12h.01', 'M12 12h.01', 'M18 12h.01'], sw: 3 },
    chevronDown: { d: ['M6 9l6 6 6-6'] },
    chevronUpDown: { d: ['M8 9.5 12 5.5l4 4', 'M8 14.5l4 4 4-4'] },
    check: { d: ['M5 12.5l4.5 4.5L19 7.5'] },
    close: { d: ['M6 6l12 12', 'M18 6 6 18'] },
    sync: { d: ['M20 11a8 8 0 0 0-14.5-3.5L4 9', 'M4 4v5h5', 'M4 13a8 8 0 0 0 14.5 3.5L20 15', 'M20 20v-5h-5'] },
    keyboard: { d: ['M3 6.5h18v11H3z', 'M7 10h.01', 'M11 10h.01', 'M15 10h.01', 'M7 14h10'], sw: 1.6 },
    wrench: { d: ['M14.5 6a4 4 0 0 0 4.8 4.8L21 12.5l-8.5 8.5a2.1 2.1 0 0 1-3-3L18 9.5', 'M14.5 6l2.5 2.5'] },
    info: { d: ['M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17z', 'M12 11v5', 'M12 8h.01'] },
    arrowUp: { d: ['M12 19V5', 'M6 11l6-6 6 6'] },
    arrowDown: { d: ['M12 5v14', 'M6 13l6 6 6-6'] },
  } satisfies Record<string, Def>

  export type IconName = keyof typeof ICONS
</script>

<script lang="ts">
  interface Props {
    name: IconName
    size?: number
  }
  let { name, size = 16 }: Props = $props()
  const def: Def = $derived(ICONS[name])
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={def.fill ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width={def.sw ?? 1.7}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#each def.d as d, i (i)}
    <path {d} />
  {/each}
</svg>
