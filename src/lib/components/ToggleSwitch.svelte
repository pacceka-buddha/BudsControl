<script lang="ts">
  interface Props {
    checked: boolean
    onchange: (checked: boolean) => void
    label: string
    disabled?: boolean
  }
  let { checked, onchange, label, disabled = false }: Props = $props()
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  class:on={checked}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  button {
    position: relative;
    width: 40px;
    height: 20px;
    padding: 0;
    border-radius: var(--radius-pill);
    border: 1px solid var(--stroke-strong);
    background: var(--fill);
    transition:
      background-color var(--dur),
      border-color var(--dur);
  }
  button:hover:not(:disabled) {
    background: var(--fill-hover);
  }
  button.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  button:disabled {
    opacity: 0.5;
  }
  .knob {
    position: absolute;
    top: 50%;
    left: 4px;
    width: 12px;
    height: 12px;
    margin-top: -6px;
    border-radius: 50%;
    background: var(--text-2);
    transition:
      transform var(--dur) var(--ease-spring),
      background-color var(--dur),
      width 120ms;
  }
  button:active:not(:disabled) .knob {
    width: 15px;
  }
  .on .knob {
    transform: translateX(20px);
    background: var(--on-accent);
  }
  .on:active:not(:disabled) .knob {
    transform: translateX(17px);
  }

  /* The flyout: a smaller switch with a white knob on a soft track. */
  :global(html.glass) button {
    width: 32px;
    height: 18px;
    border: 0;
    background: var(--track);
    box-shadow: inset 0 0 0 0.5px rgba(127, 127, 127, 0.25);
  }
  :global(html.glass) button.on {
    background: var(--accent);
  }
  :global(html.glass) .knob {
    left: 2px;
    width: 14px;
    height: 14px;
    margin-top: -7px;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
  }
  :global(html.glass) button:active:not(:disabled) .knob {
    width: 16px;
  }
  :global(html.glass) .on .knob {
    transform: translateX(14px);
  }
  :global(html.glass) .on:active:not(:disabled) .knob {
    transform: translateX(12px);
  }
</style>
