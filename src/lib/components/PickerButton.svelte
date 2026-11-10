<script lang="ts">
  // "Label ........ [ current value v ]": opens a menu of choices.
  import { popupMenu } from '../menu'
  import Icon from './Icon.svelte'

  interface Option {
    id: string
    text: string
  }
  interface Props {
    label: string
    value: string
    options: Option[]
    /** The id of the current option (gets the check mark). */
    current?: string | null
    onselect: (id: string) => void
    disabled?: boolean
  }
  let { label, value, options, current = null, onselect, disabled = false }: Props = $props()

  let button: HTMLButtonElement | undefined = $state()
  let open = $state(false)

  async function show() {
    if (!button || open) return
    open = true
    try {
      const chosen = await popupMenu(
        options.map((o) => ({ id: o.id, text: o.text, checked: o.id === current })),
        button,
      )
      if (chosen !== null && chosen !== current) onselect(chosen)
    } finally {
      open = false
    }
  }
</script>

<div class="picker">
  <span class="label">{label}</span>
  <button type="button" bind:this={button} {disabled} aria-haspopup="menu" aria-expanded={open} onclick={show}>
    <span class="value">{value}</span>
    <Icon name="chevronDown" size={12} />
  </button>
</div>

<style>
  .picker {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .label {
    font-size: 12px;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 150px;
    padding: 3px 8px 3px 10px;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-pill);
    background: var(--fill);
    color: var(--text-2);
    font-size: 12px;
    font-weight: 500;
    transition: background-color 120ms;
  }
  button:hover:not(:disabled) {
    background: var(--fill-hover);
  }
  button:active:not(:disabled),
  button[aria-expanded='true'] {
    background: var(--fill-press);
  }
  button:disabled {
    opacity: 0.5;
  }
  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
