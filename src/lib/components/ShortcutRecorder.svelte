<script lang="ts">
  // "Record Shortcut": click, then press the combination. Esc cancels, Backspace clears. A combination
  // needs a modifier (Ctrl, Alt, Shift, Win) unless it is an F-key, or it would be stolen from every app.
  // While listening, the app's own global shortcuts are switched off so the old one can't fire.
  import { onDestroy } from 'svelte'
  import { api } from '../api'
  import { display, interpret } from '../shortcut'
  import Icon from './Icon.svelte'

  interface Props {
    /** The current accelerator, if any. */
    value: string | null
    onchange: (accelerator: string | null) => void
    /** Another program already owns this combination. */
    conflict?: boolean
    label: string
  }
  let { value, onchange, conflict = false, label }: Props = $props()

  let recording = $state(false)
  let shaking = $state(false)
  let button: HTMLButtonElement | undefined = $state()

  async function start() {
    // Listen first: a key pressed right after the click must not be missed while shortcuts pause.
    recording = true
    window.addEventListener('keydown', onKey, true)
    window.addEventListener('keyup', swallow, true)
    await api.setRecording(true)
  }

  async function stop() {
    if (!recording) return
    recording = false
    window.removeEventListener('keydown', onKey, true)
    window.removeEventListener('keyup', swallow, true)
    await api.setRecording(false)
    button?.focus()
  }

  function swallow(e: KeyboardEvent) {
    e.preventDefault()
    e.stopPropagation()
  }

  function shake() {
    shaking = true
    setTimeout(() => (shaking = false), 400)
  }

  async function onKey(e: KeyboardEvent) {
    swallow(e)
    const result = interpret(e)
    switch (result.kind) {
      case 'wait':
        return
      case 'cancel':
        await stop()
        return
      case 'clear':
        onchange(null)
        await stop()
        return
      case 'reject':
        shake()
        return
      case 'accept':
        if (await api.validateShortcut(result.accelerator)) {
          onchange(result.accelerator)
          await stop()
        } else {
          shake()
        }
    }
  }

  // Never leave the shortcuts switched off.
  onDestroy(() => void stop())
</script>

<div class="recorder">
  <button
    type="button"
    bind:this={button}
    class="combo"
    class:recording
    class:shake={shaking}
    class:conflict
    aria-label="{label}: {value ? display(value) : 'no shortcut'}. Activate to record a new one."
    onclick={() => (recording ? stop() : start())}
    onblur={() => void stop()}
  >
    {#if recording}
      Type shortcut…
    {:else if value}
      <span class="keys">{display(value)}</span>
    {:else}
      Record Shortcut
    {/if}
  </button>
  {#if value && !recording}
    <button type="button" class="clear" aria-label="Clear {label} shortcut" title="Clear shortcut" onclick={() => onchange(null)}>
      <Icon name="close" size={12} />
    </button>
  {/if}
</div>

<style>
  .recorder {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .combo {
    min-width: 136px;
    padding: 5px 12px;
    border: 1px solid var(--stroke-strong);
    border-radius: var(--radius-control);
    background: var(--fill);
    font-size: 12px;
    color: var(--text-2);
    transition: background-color 120ms;
  }
  .combo:hover {
    background: var(--fill-hover);
  }
  .keys {
    color: var(--text);
    font-family: var(--font-mono);
  }
  .recording {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--fill-hover);
  }
  .conflict {
    border-color: var(--danger);
  }
  .shake {
    animation: shake 400ms;
  }
  @keyframes shake {
    20% {
      transform: translateX(-4px);
    }
    40% {
      transform: translateX(4px);
    }
    60% {
      transform: translateX(-3px);
    }
    80% {
      transform: translateX(3px);
    }
  }
  .clear {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--stroke);
    border-radius: 50%;
    background: var(--fill);
    color: var(--text-2);
  }
  .clear:hover {
    background: var(--fill-hover);
  }
</style>
