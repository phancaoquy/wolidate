import { open } from '@tauri-apps/plugin-dialog'
import * as api from '../ipc/commands'
import type { Book, BookCollection, Collection, SyncStatus } from '../ipc/commands'

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

class LibraryStore {
  books = $state<Book[]>([])
  collections = $state<Collection[]>([])
  memberships = $state<BookCollection[]>([])
  search = $state('')
  activeCollection = $state<string | null>(null)
  loading = $state(false)
  importing = $state(false)
  syncing = $state(false)
  error = $state<string | null>(null)
  syncStatus = $state<SyncStatus>({
    connected: false,
    dirty: false,
    lastSyncedAtMs: 0,
    driveFileId: null,
  })
  initialized = false

  filtered = $derived.by(() => {
    const query = this.search.trim().toLowerCase()
    const allowed = this.activeCollection
      ? new Set(
          this.memberships
            .filter((row) => row.collectionId === this.activeCollection)
            .map((row) => row.bookHash),
        )
      : null
    return this.books.filter((book) => {
      if (allowed && !allowed.has(book.bookHash)) return false
      if (!query) return true
      return `${book.title} ${book.author}`.toLowerCase().includes(query)
    })
  })

  async initialize(): Promise<void> {
    if (this.initialized) return
    this.initialized = true
    await Promise.all([this.refresh(), this.refreshSyncStatus()])
  }

  async refresh(): Promise<void> {
    if (!api.isTauri()) {
      this.error = 'Open wolidate through the Tauri app to load the library.'
      return
    }
    this.loading = true
    this.error = null
    try {
      const [books, collections, memberships] = await Promise.all([
        api.listBooks(),
        api.listCollections(),
        api.listBookCollections(),
      ])
      this.books = books
      this.collections = collections
      this.memberships = memberships
    } catch (error) {
      this.error = message(error)
    } finally {
      this.loading = false
    }
  }

  async refreshSyncStatus(): Promise<void> {
    if (!api.isTauri()) return
    try {
      this.syncStatus = await api.syncStatus()
    } catch {
      // A missing keyring or a first run is not a fatal library error.
    }
  }

  async importBook(): Promise<Book | null> {
    if (!api.isTauri()) {
      this.error = 'Import needs the Tauri app.'
      return null
    }
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: 'E-books',
          extensions: ['epub', 'mobi', 'azw', 'azw3', 'fb2', 'cbz', 'pdf'],
        },
      ],
    })
    if (!selected) return null
    const path = Array.isArray(selected) ? selected[0] : selected
    if (!path) return null
    return this.importPath(path)
  }

  async importPath(path: string): Promise<Book | null> {
    this.importing = true
    this.error = null
    try {
      const book = await api.importBook(path)
      await this.refresh()
      return book
    } catch (error) {
      this.error = message(error)
      return null
    } finally {
      this.importing = false
    }
  }

  async removeBook(book: Book): Promise<void> {
    const approved = window.confirm(`Remove "${book.title}" from the library?`)
    if (!approved) return
    try {
      await api.deleteBook(book.bookHash)
      await this.refresh()
    } catch (error) {
      this.error = message(error)
    }
  }

  async addCollection(name: string): Promise<void> {
    if (!name.trim()) return
    try {
      await api.createCollection(name.trim())
      await this.refresh()
    } catch (error) {
      this.error = message(error)
    }
  }

  async addBookToCollection(book: Book, collectionId: string): Promise<void> {
    try {
      await api.setBookCollection(book.bookHash, collectionId, true)
      await this.refresh()
    } catch (error) {
      this.error = message(error)
    }
  }

  async renameCollection(id: string, name: string): Promise<void> {
    if (!name.trim()) return
    try {
      await api.renameCollection(id, name.trim())
      await this.refresh()
    } catch (error) {
      this.error = message(error)
    }
  }

  async removeCollection(id: string): Promise<void> {
    try {
      await api.deleteCollection(id)
      if (this.activeCollection === id) this.activeCollection = null
      await this.refresh()
    } catch (error) {
      this.error = message(error)
    }
  }

  async sync(): Promise<void> {
    if (!api.isTauri() || this.syncing) return
    this.syncing = true
    this.error = null
    try {
      await api.syncNow()
      await Promise.all([this.refresh(), this.refreshSyncStatus()])
    } catch (error) {
      this.error = message(error)
    } finally {
      this.syncing = false
    }
  }

  async connectDrive(): Promise<void> {
    try {
      await api.googleOAuthStart()
    } catch (error) {
      this.error = message(error)
    }
  }

  async disconnectDrive(): Promise<void> {
    try {
      await api.googleOAuthDisconnect()
      await this.refreshSyncStatus()
    } catch (error) {
      this.error = message(error)
    }
  }
}

export const library = new LibraryStore()
