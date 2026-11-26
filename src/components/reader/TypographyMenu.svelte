<script lang="ts">
  import { reader } from '../../lib/stores/reader.svelte'
  import type { ReaderTheme } from '../../lib/stores/reader.svelte'

  const themes: Array<{ id: ReaderTheme; label: string }> = [
    { id: 'light', label: 'Light' },
    { id: 'sepia', label: 'Sepia' },
    { id: 'dark', label: 'Dark' },
  ]
</script>

<div class="typography-menu" role="dialog" aria-label="Typography">
  <header class="sidebar-header" style="padding:0 0 10px;border:0">
    <h2>Typography</h2>
    <button
      class="icon-button"
      aria-label="Close typography"
      onclick={() => (reader.typographyOpen = false)}
    >
      ×
    </button>
  </header>

  <div class="field">
    <label for="font-size">
      <span>Font size</span>
      <span>{Math.round(reader.fontSize * 100)}%</span>
    </label>
    <input
      id="font-size"
      type="range"
      min="0.8"
      max="2"
      step="0.05"
      value={reader.fontSize}
      oninput={(event) =>
        reader.setFontSize(Number((event.currentTarget as HTMLInputElement).value))}
    />
  </div>

  <div class="field">
    <label for="line-height">
      <span>Line height</span>
      <span>{reader.lineHeight.toFixed(1)}</span>
    </label>
    <input
      id="line-height"
      type="range"
      min="1.2"
      max="2.4"
      step="0.1"
      value={reader.lineHeight}
      oninput={(event) =>
        reader.setLineHeight(
          Number((event.currentTarget as HTMLInputElement).value),
        )}
    />
  </div>

  <div class="field">
    <div class="field-label">Theme</div>
    <div class="theme-row">
      {#each themes as theme (theme.id)}
        <button
          class:active={reader.theme === theme.id}
          onclick={() => reader.setTheme(theme.id)}
        >
          {theme.label}
        </button>
      {/each}
    </div>
  </div>
</div>
