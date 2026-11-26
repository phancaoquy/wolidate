<script lang="ts">
  import Icon from '../Icon.svelte'
  import { reader } from '../../lib/stores/reader.svelte'
  import type { ReaderFont, ReaderTheme } from '../../lib/stores/reader.svelte'

  const themes: Array<{ id: ReaderTheme; label: string }> = [
    { id: 'system', label: 'Auto' },
    { id: 'light', label: 'Light' },
    { id: 'sepia', label: 'Sepia' },
    { id: 'dark', label: 'Dark' },
  ]

  const fonts: Array<{ id: ReaderFont; label: string; sample: string }> = [
    { id: 'serif', label: 'Serif', sample: 'Aa' },
    { id: 'sans', label: 'Sans', sample: 'Aa' },
    { id: 'mono', label: 'Mono', sample: 'Aa' },
  ]
</script>

<div class="typography-menu" role="dialog" aria-label="Typography">
  <header class="popover-header">
    <h2>Appearance</h2>
    <button
      class="icon-button"
      aria-label="Close appearance"
      onclick={() => (reader.typographyOpen = false)}
    >
      <Icon name="close" size={16} />
    </button>
  </header>

  <div class="field">
    <div class="field-label">Theme</div>
    <div class="segmented">
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

  <div class="field">
    <div class="field-label">Typeface</div>
    <div class="segmented font-segmented">
      {#each fonts as font (font.id)}
        <button
          class:active={reader.fontFamily === font.id}
          class={`font-${font.id}`}
          onclick={() => reader.setFontFamily(font.id)}
        >
          {font.sample}
        </button>
      {/each}
    </div>
  </div>

  <div class="field">
    <label for="font-size">
      <span>Text size</span>
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
      <span>Line spacing</span>
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
</div>
