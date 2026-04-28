<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    void (async () => {
      try {
        const w = getCurrentWindow();
        if (w.label === "capture") {
          await goto("/capture", { replaceState: true });
        }
      } catch {
        /* Not running inside Tauri */
      }
    })();
  });
</script>

{@render children()}
