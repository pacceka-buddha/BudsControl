<script lang="ts">
  interface Props {
    size?: number
  }
  let { size = 16 }: Props = $props()
</script>

<span class="spinner" style:width="{size}px" style:height="{size}px" role="progressbar" aria-label="Connecting"></span>

<style>
  .spinner {
    display: inline-block;
    border-radius: 50%;
    border: 2px solid var(--track);
    border-top-color: var(--accent);
    animation: spin 800ms linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
