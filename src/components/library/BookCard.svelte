<script lang="ts">
  import { library } from '../../lib/stores/library.svelte'
  import { reader } from '../../lib/stores/reader.svelte'
  import type { Book } from '../../lib/ipc/commands'

  let { book }: { book: Book } = $props()
  let coverFailed = $state(false)

  const coverUrl = $derived(`http://book.localhost/${book.bookHash}/__cover`)
  const initials = $derived(
    book.title
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]?.toUpperCase() ?? '')
      .join(''),
  )
  const progress = $derived(
    Math.min(100, Math.max(0, book.progressFraction * 100)),
  )

  function openBook(): void {
    void reader.open(book)
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      openBook()
    }
  }

  function addToCollection(event: Event): void {
    const select = event.currentTarget as HTMLSelectElement
    if (!select.value) return
    void library.addBookToCollection(book, select.value)
    select.value = ''
  }
</script>

<div
  class="book-card"
  role="button"
  tabindex="0"
  onclick={openBook}
  onkeydown={onKeydown}
  aria-label={`Open ${book.title}`}
>
  <div class="cover-wrap">
    {#if coverFailed}
      <div class="cover-placeholder">{initials || '?'}</div>
    {:else}
      <img
        src={coverUrl}
        alt=""
        loading="lazy"
        onerror={() => (coverFailed = true)}
      />
    {/if}
    {#if progress > 0}
      <div class="book-progress"><span style={`width:${progress}%`}></span></div>
    {/if}
  </div>

  <div class="book-info">
    <div class="book-text">
      <div class="book-title">{book.title}</div>
      <div class="book-author">{book.author || 'Unknown author'}</div>
    </div>
    <button
      class="book-delete"
      aria-label={`Remove ${book.title}`}
      title="Remove from library"
      onclick={(event) => {
        event.stopPropagation()
        void library.removeBook(book)
      }}
    >
      ×
    </button>
  </div>

  {#if library.collections.length > 0}
    <select
      class="book-collection"
      aria-label={`Add ${book.title} to a collection`}
      onclick={(event) => event.stopPropagation()}
      onchange={addToCollection}
    >
      <option value="">+ Collection</option>
      {#each library.collections as collection (collection.id)}
        <option value={collection.id}>{collection.name}</option>
      {/each}
    </select>
  {/if}
</div>
