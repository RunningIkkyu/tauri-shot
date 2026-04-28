<script lang="ts">
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="capture-shell">
  {@render children()}
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    overscroll-behavior: none;
    background: #000;
    scrollbar-width: none;
  }

  :global(html)::-webkit-scrollbar,
  :global(body)::-webkit-scrollbar {
    display: none;
  }

  .capture-shell {
    position: fixed;
    inset: 0;
    margin: 0;
    padding: 0;
    overflow: hidden;
    background: #000;
    box-sizing: border-box;
  }
</style>
