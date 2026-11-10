<script lang="ts">
  // A vertical slider for one EQ band. Dragging reports every step through `onchange`; the new curve
  // is sent to the buds on release (`oncommit`). Keyboard: arrows +-1, PageUp / PageDown +-3, Home / End.
  import { gainFromPosition } from '../format'

  interface Props {
    value: number
    min: number
    max: number
    label: string
    /** The value, spoken: "3 decibels". */
    valueText: string
    height?: number
    onchange: (value: number) => void
    oncommit: () => void
  }
  let { value, min, max, label, valueText, height = 64, onchange, oncommit }: Props = $props()

  let track: HTMLElement | undefined = $state()
  let dragging = false

  // Positions in px from the top of the track.
  const span = $derived(Math.max(max - min, 1))
  const y = (v: number) => (height * (max - v)) / span
  const zeroY = $derived(y(0))
  const valueY = $derived(y(value))

  function update(event: PointerEvent) {
    if (!track) return
    const rect = track.getBoundingClientRect()
    const next = gainFromPosition(event.clientY - rect.top, rect.height, min, max)
    if (next !== value) onchange(next)
  }

  function down(event: PointerEvent) {
    dragging = true
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    update(event)
  }
  function move(event: PointerEvent) {
    if (dragging) update(event)
  }
  function up() {
    if (!dragging) return
    dragging = false
    oncommit()
  }

  function key(event: KeyboardEvent) {
    const step: Record<string, number> = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1, PageUp: 3, PageDown: -3 }
    let next = value
    if (event.key in step) next = value + step[event.key]
    else if (event.key === 'Home') next = min
    else if (event.key === 'End') next = max
    else return
    event.preventDefault()
    next = Math.min(Math.max(next, min), max)
    if (next !== value) {
      onchange(next)
      oncommit()
    }
  }
</script>

<div
  class="slider"
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-orientation="vertical"
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-valuetext={valueText}
  style:height="{height}px"
  bind:this={track}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
>
  <span class="groove"></span>
  <span class="fill" style:top="{Math.min(valueY, zeroY)}px" style:height="{Math.abs(valueY - zeroY)}px"></span>
  <span class="knob" style:top="{valueY - 6}px"></span>
</div>

<style>
  .slider {
    position: relative;
    width: 100%;
    touch-action: none;
    border-radius: var(--radius-control);
  }
  .groove,
  .fill {
    position: absolute;
    left: 50%;
    width: 4px;
    margin-left: -2px;
    border-radius: 2px;
  }
  .groove {
    top: 0;
    bottom: 0;
    background: var(--track);
  }
  .fill {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    left: 50%;
    width: 12px;
    height: 12px;
    margin-left: -6px;
    border-radius: 50%;
    background: var(--knob);
    box-shadow: var(--shadow-knob);
    transition: top 100ms var(--ease-out);
  }
  .slider:active .knob {
    transition: none;
  }
</style>
