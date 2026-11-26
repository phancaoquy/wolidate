<script lang="ts">
  import { reader } from '../../lib/stores/reader.svelte'
  import type { TocItem } from '../../lib/stores/reader.svelte'

  let query = $state('')

  const title = $derived(
    reader.sidebarTab === 'toc'
      ? 'Contents'
      : reader.sidebarTab === 'notes'
        ? 'Notes & highlights'
        : 'Search',
  )

  const flatToc = $derived.by(() => {
    const result: Array<TocItem & { depth: number }> = []
    const walk = (items: TocItem[], depth: number): void => {
      for (const item of items) {
        result.push({ ...item, depth })
        if (item.subitems?.length) walk(item.subitems, depth + 1)
      }
    }
    walk(reader.toc, 0)
    return result
  })

  const searchResults = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return []
    const toc = flatToc
      .filter((item) => item.label.toLowerCase().includes(needle))
      .slice(0, 40)
      .map((item) => ({
        key: `toc:${item.href}`,
        label: item.label,
        detail: 'Contents',
        target: item.href,
      }))
    const notes = reader.annotations
      .filter((annotation) =>
        `${annotation.quoteText} ${annotation.noteText}`
          .toLowerCase()
          .includes(needle),
      )
      .slice(0, 40)
      .map((annotation) => ({
        key: `note:${annotation.id}`,
        label: annotation.quoteText || annotation.noteText,
        detail: 'Highlight',
        target: annotation.cfi,
      }))
    return [...toc, ...notes]
  })

  function go(target: string): void {
    reader.requestNavigation(target)
    if (window.matchMedia('(max-width: 767px)').matches) {
      reader.sidebarTab = null
    }
  }
</script>

<aside class="sidebar" aria-label={title}>
  <header class="sidebar-header">
    <h2>{title}</h2>
    <button
      class="icon-button"
      aria-label="Close panel"
      onclick={() => (reader.sidebarTab = null)}
    >
      ×
    </button>
  </header>

  {#if reader.sidebarTab === 'toc'}
    <div class="sidebar-scroll">
      {#if flatToc.length === 0}
        <p class="sidebar-empty">This book has no table of contents.</p>
      {:else}
        {#each flatToc as item (item.href + item.label)}
          <button
            class={`toc-item depth-${Math.min(item.depth, 2)}`}
            onclick={() => go(item.href)}
          >
            {item.label}
          </button>
        {/each}
      {/if}
    </div>
  {:else if reader.sidebarTab === 'notes'}
    <div class="sidebar-scroll">
      {#if reader.annotations.length === 0}
        <p class="sidebar-empty">
          Select text and choose Highlight to keep a note here.
        </p>
      {:else}
        {#each reader.annotations as annotation (annotation.id)}
          <div class="note-item">
            <button class="note-quote" onclick={() => go(annotation.cfi)}>
              {annotation.quoteText || 'Bookmark'}
            </button>
            {#if annotation.noteText}
              <p class="note-text">{annotation.noteText}</p>
            {/if}
            <div class="note-meta">
              <span>{annotation.kind}</span>
              <button
                onclick={() => void reader.removeAnnotation(annotation.id)}
              >
                Remove
              </button>
            </div>
          </div>
        {/each}
      {/if}
    </div>
  {:else}
    <div class="sidebar-search">
      <input
        type="search"
        placeholder="Search contents and notes"
        bind:value={query}
        aria-label="Search within this book"
      />
    </div>
    <div class="sidebar-scroll">
      {#if !query.trim()}
        <p class="sidebar-empty">
          Search chapter titles and saved highlights.
        </p>
      {:else if searchResults.length === 0}
        <p class="sidebar-empty">No matches in this book.</p>
      {:else}
        {#each searchResults as result (result.key)}
          <button class="search-result" onclick={() => go(result.target)}>
            <span class="search-result-label">{result.label}</span>
            <span class="note-meta">{result.detail}</span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</aside>
