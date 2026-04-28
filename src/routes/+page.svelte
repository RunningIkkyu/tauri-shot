<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type AppSettings = {
    screenshotShortcut: string;
    defaultExportFormat: string;
  };

  let screenshotShortcut = $state("cmd+shift+2");
  let defaultExportFormat = $state<"png" | "jpeg" | "webp">("webp");
  let status = $state("");
  let loading = $state(true);

  onMount(async () => {
    try {
      const s = await invoke<AppSettings>("get_settings");
      screenshotShortcut = s.screenshotShortcut;
      const f = s.defaultExportFormat?.toLowerCase() ?? "png";
      defaultExportFormat =
        f === "jpeg" || f === "jpg" ? "jpeg" : f === "webp" ? "webp" : "png";
    } catch (e) {
      status = String(e);
    } finally {
      loading = false;
    }
  });

  async function save(event: Event) {
    event.preventDefault();
    status = "";
    try {
      await invoke("save_settings", {
        settings: {
          screenshotShortcut,
          defaultExportFormat,
        },
      });
      status = "Saved";
    } catch (e) {
      status = String(e);
    }
  }
</script>

<main class="dashboard">
  <h1>tauri-shot Settings</h1>
  <p class="lead">
    Use the global shortcut to capture a region; after selecting, annotate and export.
  </p>

  {#if loading}
    <p>Loading…</p>
  {:else}
    <form class="card" onsubmit={save}>
      <label class="field">
        <span class="label">Screenshot shortcut</span>
        <input
          type="text"
          bind:value={screenshotShortcut}
          placeholder="cmd+shift+2"
          autocomplete="off"
        />
        <span class="hint">
          Examples: <code>cmd+shift+2</code>, <code>ctrl+shift+s</code>. Modifiers:
          <code>cmd</code> / <code>ctrl</code> / <code>shift</code> / <code>alt</code>
        </span>
      </label>

      <label class="field">
        <span class="label">Default export format</span>
        <select bind:value={defaultExportFormat}>
          <option value="png">PNG</option>
          <option value="jpeg">JPG</option>
          <option value="webp">WebP</option>
        </select>
      </label>

      <button type="submit" class="primary">Save settings</button>
      {#if status}
        <p class="status" class:error={status !== "Saved"}>{status}</p>
      {/if}
    </form>
  {/if}
</main>

<style>
  :global(html, body) {
    margin: 0;
    min-height: 100%;
    font-family:
      system-ui,
      -apple-system,
      Segoe UI,
      Roboto,
      sans-serif;
    background: #0f1115;
    color: #e8eaed;
  }

  .dashboard {
    max-width: 520px;
    margin: 0 auto;
    padding: 2.5rem 1.25rem 4rem;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
    letter-spacing: -0.02em;
  }

  .lead {
    margin: 0 0 1.75rem;
    color: #9aa0a6;
    line-height: 1.5;
    font-size: 0.95rem;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    padding: 1.5rem;
    border-radius: 12px;
    background: #1a1d24;
    border: 1px solid #2d3139;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .label {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #9aa0a6;
  }

  input,
  select {
    border-radius: 8px;
    border: 1px solid #3c4043;
    padding: 0.65rem 0.75rem;
    font-size: 1rem;
    background: #0f1115;
    color: #e8eaed;
    outline: none;
  }

  input:focus,
  select:focus {
    border-color: #8ab4f8;
    box-shadow: 0 0 0 2px rgba(138, 180, 248, 0.25);
  }

  .hint {
    font-size: 0.8rem;
    color: #6f7378;
    line-height: 1.45;
  }

  .hint code {
    font-size: 0.78rem;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    background: #0f1115;
    color: #c4c7c5;
  }

  .primary {
    align-self: flex-start;
    border: none;
    border-radius: 8px;
    padding: 0.65rem 1.25rem;
    font-weight: 600;
    font-size: 0.95rem;
    cursor: pointer;
    background: linear-gradient(180deg, #8ab4f8, #669df6);
    color: #0f1115;
  }

  .primary:hover {
    filter: brightness(1.05);
  }

  .status {
    margin: 0;
    font-size: 0.9rem;
    color: #81c995;
  }

  .status.error {
    color: #f28b82;
  }
</style>
