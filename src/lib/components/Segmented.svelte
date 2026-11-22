<script lang="ts" generics="T extends { id: string; label: string }">
  // A segmented control, after the original macOS app: one frosted pill that follows the selected
  // segment on a spring, lifts and stretches as it moves, then settles, while the newly selected
  // label swells a little.
  //   variant "expand": unselected segments are icon-only squares, the selected one grows to show its label
  //   variant "even":   segments share the width (or size to their labels with `fit`)
  // Radio-group semantics: arrow keys move the selection, Home / End jump, only one tab stop.
  import { untrack, type Snippet } from 'svelte'

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
  let blob: HTMLElement | undefined = $state()
  let buttons: (HTMLElement | undefined)[] = $state([])
  let ready = $state(false)

  const selectedIndex = $derived(items.findIndex((i) => i.id === selected))
  // Roving tab stop: the selected segment, or the first when nothing is selected.
  const tabStop = $derived(selectedIndex >= 0 ? selectedIndex : 0)

  const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches

  // The pill's position and width, each a spring (value and velocity) chasing the selected segment.
  const x = { at: 0, v: 0 }
  const w = { at: 0, v: 0 }
  // Response ~0.42 s with a little overshoot, like the original's spring.
  const STIFFNESS = 340
  const DAMPING = 24

  function step(s: { at: number; v: number }, target: number, dt: number) {
    s.v += (-STIFFNESS * (s.at - target) - DAMPING * s.v) * dt
    s.at += s.v * dt
  }

  function target(): { x: number; w: number } | null {
    const el = selectedIndex >= 0 ? buttons[selectedIndex] : undefined
    return el && el.offsetWidth > 0 ? { x: el.offsetLeft, w: el.offsetWidth } : null
  }

  function draw() {
    if (!pill) return
    pill.style.transform = `translateX(${x.at}px)`
    pill.style.width = `${w.at}px`
  }

  function snap() {
    const t = target()
    if (!t) {
      ready = false
      return
    }
    x.at = t.x
    w.at = t.w
    x.v = w.v = 0
    draw()
    ready = true
  }

  // Segments change width while they animate, so the target is measured again every frame.
  let frame = 0
  let until = 0
  let last = 0
  // A spring flight is running (the pill is moving on its own, so the layout must not snap it).
  let springing = false
  // The loop that keeps the pill on its segment while the layout settles.
  let followFrame = 0

  /** Snaps the pill to its segment every frame for `ms`: the layout (a segment growing on a transition)
   *  keeps moving after the selection changes, and a hidden window gets no frames until it is shown. */
  function follow(ms: number) {
    cancelAnimationFrame(followFrame)
    const end = performance.now() + ms
    const tick = () => {
      if (!springing) snap()
      followFrame = performance.now() < end ? requestAnimationFrame(tick) : 0
    }
    followFrame = requestAnimationFrame(tick)
  }

  const settleMs = () => (variant === 'expand' ? 600 : 80)

  function animate(now: number) {
    const t = target()
    if (t) {
      // Real elapsed time, capped so a stalled frame doesn't fling the pill.
      const dt = Math.min((now - last) / 1000, 1 / 30)
      step(x, t.x, dt)
      step(w, t.w, dt)
      draw()
    }
    last = now
    if (now < until) frame = requestAnimationFrame(animate)
    else {
      frame = 0
      springing = false
      snap()
      follow(settleMs())
    }
  }

  function restart(el: Element | undefined, cls: string) {
    if (!el) return
    el.classList.remove(cls)
    void (el as HTMLElement).offsetWidth
    el.classList.add(cls)
  }

  let previous: string | null = null
  $effect(() => {
    void items.length
    const changed = previous !== null && selected !== previous
    previous = selected
    if (!changed || !untrack(() => ready) || reducedMotion()) {
      // First show, a list change or reduced motion: no flight, just follow the layout settling.
      cancelAnimationFrame(frame)
      frame = 0
      springing = false
      snap()
      follow(settleMs())
      return () => {
        cancelAnimationFrame(followFrame)
        followFrame = 0
      }
    }
    restart(blob, 'bubble')
    restart(buttons[selectedIndex], 'swell')
    until = performance.now() + 800
    last = performance.now()
    cancelAnimationFrame(frame)
    cancelAnimationFrame(followFrame)
    springing = true
    frame = requestAnimationFrame(animate)
    return () => {
      cancelAnimationFrame(frame)
      frame = 0
      springing = false
    }
  })

  // A window that was hidden when the selection changed gets its frames, and its transitions, only
  // once it is shown again: keep the pill on its segment while that catches up.
  function shown() {
    if (!document.hidden) follow(settleMs() + 400)
  }
  $effect(() => {
    document.addEventListener('visibilitychange', shown)
    window.addEventListener('focus', shown)
    return () => {
      document.removeEventListener('visibilitychange', shown)
      window.removeEventListener('focus', shown)
      cancelAnimationFrame(followFrame)
    }
  })

  // The pill follows the layout whenever it settles on its own: the group resizing, or a segment
  // changing width (the "expand" variant grows the selected one on a transition). A change made while
  // the window was hidden gets no animation frames, so the pill is measured when the transition has
  // only just started; it must be measured again as the segments reach their final widths.
  $effect(() => {
    if (!group) return
    const observer = new ResizeObserver(() => {
      if (!springing) snap()
    })
    observer.observe(group)
    for (const button of buttons) if (button) observer.observe(button)
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
  <span class="pill" class:ready bind:this={pill} aria-hidden="true"><span class="blob" bind:this={blob}></span></span>
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
      <span class="content">{@render segment(item, on)}</span>
    </button>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: flex;
    padding: 3px;
    background: var(--track);
    box-shadow: inset 0 0 0 0.5px rgba(127, 127, 127, 0.16);
    border-radius: var(--radius-pill);
  }
  .pill {
    position: absolute;
    left: 0;
    top: 3px;
    height: var(--h);
    opacity: 0;
    pointer-events: none;
  }
  .pill.ready {
    opacity: 1;
  }
  .blob {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: var(--radius-pill);
    background: var(--sel);
    box-shadow:
      inset 0 0.75px 0 var(--sel-hi),
      0 1px 2px rgba(0, 0, 0, 0.15);
  }
  /* Lifts and stretches, then settles. */
  .blob:global(.bubble) {
    animation: bubble 620ms cubic-bezier(0.3, 0.7, 0.3, 1);
  }
  @keyframes bubble {
    22% {
      transform: scale(1.08, 1.3);
      filter: brightness(1.18);
      box-shadow:
        inset 0 0.75px 0 var(--sel-hi),
        0 3px 7px rgba(0, 0, 0, 0.32);
    }
  }
  .item {
    position: relative;
    z-index: 1;
    flex: 1 1 0;
    min-width: 0;
    height: var(--h);
    padding: 0 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    transition:
      background-color 120ms,
      color 160ms,
      flex-grow 420ms var(--ease-spring);
  }
  .item:hover:not(.on) {
    background: var(--fill-hover);
  }
  .item.on {
    color: var(--text);
    font-weight: 600;
  }
  .content {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 0;
  }
  /* The newly selected label swells as if seen through the bubble. */
  .item:global(.swell) .content {
    animation: swell 500ms cubic-bezier(0.3, 0.7, 0.3, 1);
  }
  @keyframes swell {
    25% {
      transform: scale(1.12);
    }
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
  @media (prefers-reduced-motion: reduce) {
    .blob:global(.bubble),
    .item:global(.swell) .content {
      animation: none;
    }
  }
</style>
