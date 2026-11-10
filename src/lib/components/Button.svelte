<script lang="ts">
  import type { Snippet } from 'svelte'

  interface Props {
    onclick: (event: MouseEvent) => void
    variant?: 'default' | 'accent' | 'danger'
    disabled?: boolean
    children: Snippet
  }
  let { onclick, variant = 'default', disabled = false, children }: Props = $props()
</script>

<button type="button" class={variant} {disabled} {onclick}>{@render children()}</button>

<style>
  button {
    padding: 4px 14px;
    min-height: 26px;
    border: 1px solid var(--stroke-strong);
    border-radius: var(--radius-control);
    background: var(--fill);
    font-size: 12px;
    transition: background-color 120ms;
  }
  button:hover:not(:disabled) {
    background: var(--fill-hover);
  }
  button:active:not(:disabled) {
    background: var(--fill-press);
  }
  button:disabled {
    opacity: 0.5;
  }
  .accent {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .accent:hover:not(:disabled) {
    background: var(--accent);
    filter: brightness(1.1);
  }
  .danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .danger:hover:not(:disabled) {
    background: var(--danger);
    filter: brightness(1.1);
  }
</style>
