<script lang="ts" generics="T extends { id: string; label: string }">
  // A segmented control: one accent pill slides to the selected segment.
  //   variant "expand": unselected segments are icon-only squares, the selected one grows to show its label
  //   variant "even":   segments share the width (or size to their labels with `fit`)
  // Radio-group semantics: arrow keys move the selection, Home / End jump, only one tab stop.
  import type { Snippet } from 'svelte'

  interface Props {
    items: T[]
    selected: string | null
    onselect: (id: T['id']) => void
    /** Accessible name of the group. */
    label: string
    segment: Snippet<[T, boolean]>
    variant?: 'expand' | 'even'
    height?: number
    /** "even": size segments to their labels instead of equal widths. */
    fit?: boolean
  }
  let { items, selected, onselect, label, segment, variant = 'even', height = 30, fit = false }: Props = $props()

  let group: HTMLElement | undefined = $state()
  let pill: HTMLElement | undefined = $state()
  let buttons: (HTMLElement | undefined)[] = $state([])
  let ready = $state(false)

  const selectedIndex = $derived(items.findIndex((i) => i.id === selected))
  // Roving tab stop: the selected segment, or the first when nothing is selected.
  const tabStop = $derived(selectedIndex >= 0 ? selectedIndex : 0)

  function place() {
    const el = selectedIndex >= 0 ? buttons[selectedIndex] : undefined
    if (!pill || !el) {
      ready = false
      return
    }
    pill.style.transform = `translate(${el.offsetLeft}px, ${el.offsetTop}px)`
    pill.style.width = `${el.offsetWidth}px`
    pill.style.height = `${el.offsetHeight}px`
    ready = true
  }

  // Segments change width while they animate, so the pill follows them frame by frame.
  let frame = 0
  function follow(ms: number) {
    cancelAnimationFrame(frame)
    const end = performance.now() + ms
    const step = () => {
      place()
      if (performance.now() < end) frame = requestAnimationFrame(step)
    }
    step()
  }

  $effect(() => {
    void selected
    void items.length
    follow(variant === 'expand' ? 420 : 60)
    return () => cancelAnimationFrame(frame)
  })

  $effect(() => {
    if (!group) return
    const observer = new ResizeObserver(() => place())
    observer.observe(group)
    return () => observer.disconnect()
  })

  function key(event: KeyboardEvent) {
    if (items.length === 0) return
    let next = -1
    const from = selectedIndex >= 0 ? selectedIndex : 0
    switch (event.key) {
      case 'ArrowRight':
      case 'ArrowDown':
        next = (from + 1) % items.length
        break
      case 'ArrowLeft':
      case 'ArrowUp':
        next = (from - 1 + items.length) % items.length
        break
      case 'Home':
        next = 0
        break
      case 'End':
        next = items.length - 1
        break
      default:
        return
    }
    event.preventDefault()
    onselect(items[next].id)
    buttons[next]?.focus()
  }
</script>

<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
  class="seg"
  class:expand={variant === 'expand'}
  class:fit
  role="radiogroup"
  aria-label={label}
  bind:this={group}
  onkeydown={key}
  style:--h="{height}px"
>
  <span class="pill" class:ready bind:this={pill} aria-hidden="true"></span>
  {#each items as item, i (item.id)}
    {@const on = item.id === selected}
    <button
      type="button"
      role="radio"
      aria-checked={on}
      aria-label={item.label}
      title={item.label}
      tabindex={i === tabStop ? 0 : -1}
      class="item"
      class:on
      bind:this={buttons[i]}
      onclick={() => !on && onselect(item.id)}
    >
      {@render segment(item, on)}
    </button>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: flex;
    padding: 3px;
    background: var(--track);
    border: 1px solid var(--stroke);
    border-radius: var(--radius-pill);
  }
  .pill {
    position: absolute;
    left: 0;
    top: 0;
    border-radius: var(--radius-pill);
    background: var(--accent);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    opacity: 0;
    pointer-events: none;
  }
  .pill.ready {
    opacity: 1;
  }
  .item {
    position: relative;
    z-index: 1;
    flex: 1 1 0;
    min-width: 0;
    height: var(--h);
    padding: 0 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    transition:
      background-color 120ms,
      color 160ms,
      flex-grow var(--dur) var(--ease-spring);
  }
  .item:hover:not(.on) {
    background: var(--fill-hover);
  }
  .item.on {
    color: var(--on-accent);
    font-weight: 600;
  }
  /* expand: the unselected modes are icon-only squares, the selected one takes the rest. */
  .expand .item {
    flex: 0 0 40px;
  }
  .expand .item.on {
    flex: 1 1 40px;
  }
  .fit .item {
    flex: 1 1 auto;
  }
</style>
