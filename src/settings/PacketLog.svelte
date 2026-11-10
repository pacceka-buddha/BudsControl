<script lang="ts">
  // The live packet log: every frame sent and received and every connection event.
  import { onMount, tick } from 'svelte'
  import { api } from '../lib/api'
  import Button from '../lib/components/Button.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import ToggleSwitch from '../lib/components/ToggleSwitch.svelte'
  import { copyText } from '../lib/clipboard'
  import { formatClock } from '../lib/format'
  import { app, resetLog } from '../lib/stores.svelte'

  let list: HTMLElement | undefined = $state()
  let paused = $state(false)
  let copied = $state(false)
  /** Follow new lines, unless the user has scrolled up to read. */
  let follow = true

  onMount(() => {
    void api.getLogPaused().then((p) => (paused = p))
  })

  function scrolled() {
    if (list) follow = list.scrollTop + list.clientHeight >= list.scrollHeight - 8
  }

  $effect(() => {
    void app.log.length
    if (follow && list) void tick().then(() => list && (list.scrollTop = list.scrollHeight))
  })

  async function copy() {
    await copyText(await api.exportLog())
    copied = true
    setTimeout(() => (copied = false), 1500)
  }

  async function clear() {
    await api.clearLog()
    resetLog()
  }
</script>

<div class="log" role="log" aria-label="Packet log" bind:this={list} onscroll={scrolled}>
  {#each app.log as entry, i (i)}
    <div class="line {entry.direction}">
      <span class="time">{formatClock(entry.timeMs)}</span>
      <span class="dir">
        {#if entry.direction === 'sent'}<Icon name="arrowUp" size={12} />{:else if entry.direction === 'received'}<Icon name="arrowDown" size={12} />{:else}<Icon name="info" size={12} />{/if}
      </span>
      <span class="text">{entry.text}</span>
    </div>
  {:else}
    <div class="empty">Nothing logged yet.</div>
  {/each}
</div>

<div class="bar">
  <label class="pause">
    <ToggleSwitch label="Pause" checked={paused} onchange={(v) => { paused = v; void api.setLogPaused(v) }} />
    Pause
  </label>
  <span class="spacer"></span>
  <Button onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
  <Button onclick={clear}>Clear</Button>
</div>

<style>
  .log {
    height: 220px;
    overflow: auto;
    padding: 8px;
    background: var(--card);
    border: 1px solid var(--stroke);
    border-radius: var(--radius-card);
    font-family: var(--font-mono);
    font-size: 11px;
    user-select: text;
    cursor: text;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    white-space: nowrap;
  }
  .time {
    color: var(--text-3);
  }
  .dir {
    display: grid;
    width: 12px;
    align-self: center;
  }
  .sent .dir {
    color: #3b82f6;
  }
  .received .dir {
    color: var(--success);
  }
  .info .dir,
  .info .text {
    color: var(--text-2);
  }
  .empty {
    color: var(--text-3);
    font-family: var(--font);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
  .pause {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
</style>
