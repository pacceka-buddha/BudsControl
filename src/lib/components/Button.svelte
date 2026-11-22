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

  /* The flyout: a glass capsule. */
  :global(html.glass) button {
    min-height: 24px;
    padding: 3px 13px;
    border: 0;
    border-radius: var(--radius-pill);
    background: var(--fill);
    box-shadow:
      inset 0 0.5px 0 var(--sel-hi),
      0 1px 2px rgba(0, 0, 0, 0.14);
    font-size: 11.5px;
    font-weight: 600;
    transition: filter 120ms;
  }
  :global(html.glass) button:hover:not(:disabled) {
    background: var(--fill);
    filter: brightness(1.12);
  }
  :global(html.glass) .danger,
  :global(html.glass) .danger:hover:not(:disabled) {
    background: rgba(255, 69, 58, 0.88);
    color: #fff;
  }
</style>
