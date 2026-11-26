<script lang="ts">
  import Icon from '../Icon.svelte'
  import { library } from '../../lib/stores/library.svelte'
  import { reader } from '../../lib/stores/reader.svelte'
  import type { Book } from '../../lib/ipc/commands'

  let { book }: { book: Book } = $props()
  let coverFailed = $state(false)
  let menuOpen = $state(false)

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
    if (menuOpen) {
      menuOpen = false
      return
    }
    void reader.open(book)
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      openBook()
    }
  }

  function addToCollection(collectionId: string): void {
    menuOpen = false
    void library.addBookToCollection(book, collectionId)
  }

  function removeBook(): void {
    menuOpen = false
    void library.removeBook(book)
  }
</script>

<div
  class="book-card"
  class:menu-open={menuOpen}
  role="button"
  tabindex="0"
  onclick={openBook}
  onkeydown={onKeydown}
  aria-label={`Open ${book.title}`}
>
  <div class="cover-wrap">
    {#if coverFailed}
      <div class="cover-placeholder">
        <span>{initials || '?'}</span>
        <span class="cover-format">{book.format.toUpperCase()}</span>
      </div>
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

    <button
      class="card-menu-button"
      aria-label={`Actions for ${book.title}`}
      aria-expanded={menuOpen}
      title="Actions"
      onclick={(event) => {
        event.stopPropagation()
        menuOpen = !menuOpen
      }}
    >
      <Icon name="more" size={18} />
    </button>

    {#if menuOpen}
      <div class="card-menu">
        {#each library.collections as collection (collection.id)}
          <button
            onclick={(event) => {
              event.stopPropagation()
              addToCollection(collection.id)
            }}
          >
            <Icon name="collection" size={15} />
            Add to {collection.name}
          </button>
        {/each}
        <button
          class="danger"
          onclick={(event) => {
            event.stopPropagation()
            removeBook()
          }}
        >
          <Icon name="trash" size={15} />
          Remove from library
        </button>
      </div>
    {/if}
  </div>

  <div class="book-info">
    <div class="book-title">{book.title}</div>
    <div class="book-author">{book.author || 'Unknown author'}</div>
  </div>
</div>
