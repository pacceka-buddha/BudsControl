<script lang="ts">
  // One line of a settings page: a title, an optional description, and a control on the right.
  import type { Snippet } from 'svelte'

  interface Props {
    title: string
    description?: string
    children?: Snippet
  }
  let { title, description, children }: Props = $props()
</script>

<div class="row">
  <div class="text">
    <div class="title">{title}</div>
    {#if description}<div class="desc">{description}</div>{/if}
  </div>
  {#if children}<div class="control">{@render children()}</div>{/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 48px;
    padding: 10px 14px;
    background: var(--card);
    border: 1px solid var(--stroke);
    border-radius: var(--radius-card);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 13px;
  }
  .desc {
    margin-top: 1px;
    font-size: 12px;
    color: var(--text-2);
  }
  .control {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
