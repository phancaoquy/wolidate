<script lang="ts">
  import { reader } from '../../lib/stores/reader.svelte'

  const selection = $derived(reader.selection)

  function highlight(): void {
    if (!selection) return
    void reader.addAnnotation({
      kind: 'highlight',
      cfi: selection.cfi,
      quoteText: selection.text,
      color: 'yellow',
    })
  }

  function note(): void {
    if (!selection) return
    const text = window.prompt('Note', '')
    if (text === null) return
    void reader.addAnnotation({
      kind: 'highlight',
      cfi: selection.cfi,
      quoteText: selection.text,
      noteText: text,
      color: 'yellow',
    })
  }

  async function copy(): Promise<void> {
    if (!selection) return
    await navigator.clipboard.writeText(selection.text)
    reader.setSelection(null)
  }
</script>

{#if selection}
  <div
    class="selection-popover"
    style={`left:${selection.x}px;top:${selection.y}px`}
    role="toolbar"
    aria-label="Selection actions"
  >
    <button onclick={highlight}>Highlight</button>
    <button onclick={note}>Note</button>
    <button onclick={() => void copy()}>Copy</button>
  </div>
{/if}
