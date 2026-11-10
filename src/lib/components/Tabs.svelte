<script lang="ts" generics="T extends { id: string; label: string }">
  // A row of tabs (pivot): the selected one has an accent underline. Arrow keys move between tabs.
  interface Props {
    tabs: T[]
    selected: string
    onselect: (id: T['id']) => void
    label: string
  }
  let { tabs, selected, onselect, label }: Props = $props()

  let buttons: (HTMLElement | undefined)[] = $state([])

  function key(event: KeyboardEvent) {
    const from = tabs.findIndex((t) => t.id === selected)
    let next = from
    if (event.key === 'ArrowRight') next = (from + 1) % tabs.length
    else if (event.key === 'ArrowLeft') next = (from - 1 + tabs.length) % tabs.length
    else if (event.key === 'Home') next = 0
    else if (event.key === 'End') next = tabs.length - 1
    else return
    event.preventDefault()
    onselect(tabs[next].id)
    buttons[next]?.focus()
  }
</script>

<div class="tabs" role="tablist" aria-label={label} tabindex="-1" onkeydown={key}>
  {#each tabs as tab, i (tab.id)}
    <button
      type="button"
      role="tab"
      id="tab-{tab.id}"
      aria-selected={tab.id === selected}
      aria-controls="panel-{tab.id}"
      tabindex={tab.id === selected ? 0 : -1}
      class:on={tab.id === selected}
      bind:this={buttons[i]}
      onclick={() => onselect(tab.id)}
    >
      {tab.label}
    </button>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--stroke);
  }
  button {
    position: relative;
    padding: 8px 14px 10px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
    border-radius: var(--radius-control) var(--radius-control) 0 0;
  }
  button:hover {
    background: var(--fill);
  }
  button.on {
    color: var(--text);
    font-weight: 600;
  }
  button.on::after {
    content: '';
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: -1px;
    height: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
</style>
