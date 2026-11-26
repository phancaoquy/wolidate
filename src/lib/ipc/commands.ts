import { invoke } from '@tauri-apps/api/core'

export interface Book {
  bookHash: string
  sourceUri: string
  fileSize: number
  format: string
  title: string
  author: string
  language: string
  coverRelPath: string | null
  addedAtMs: number
  deletedAtMs: number | null
  progressFraction: number
  progressCfi: string | null
}

export interface Progress {
  bookHash: string
  cfi: string
  fraction: number
  updatedAtMs: number
  deviceId: string
}

export type AnnotationKind = 'highlight' | 'bookmark'

export interface Annotation {
  id: string
  bookHash: string
  kind: AnnotationKind
  cfi: string
  color: string
  quoteText: string
  noteText: string
  createdAtMs: number
  updatedAtMs: number
  deletedAtMs: number | null
}

export interface AnnotationInput {
  id?: string | null
  bookHash: string
  kind: AnnotationKind
  cfi: string
  color?: string
  quoteText?: string
  noteText?: string
  createdAtMs?: number
}

export interface Collection {
  id: string
  name: string
  updatedAtMs: number
  deletedAtMs: number | null
}

export interface BookCollection {
  bookHash: string
  collectionId: string
  updatedAtMs: number
  deletedAtMs: number | null
}

export interface SyncStatus {
  connected: boolean
  dirty: boolean
  lastSyncedAtMs: number
  driveFileId: string | null
}

export interface SyncRun {
  downloaded: boolean
  uploaded: boolean
  remoteFound: boolean
  lastSyncedAtMs: number
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

function requireTauri(): void {
  if (!isTauri()) {
    throw new Error('This action needs the wolidate desktop or Android shell.')
  }
}

export async function listBooks(
  search: string | null = null,
  collectionId: string | null = null,
): Promise<Book[]> {
  requireTauri()
  return invoke<Book[]>('list_books', { search, collectionId })
}

export async function getBook(bookHash: string): Promise<Book | null> {
  requireTauri()
  return invoke<Book | null>('get_book', { bookHash })
}

export async function importBook(path: string): Promise<Book> {
  requireTauri()
  return invoke<Book>('import_book', { path })
}

export async function deleteBook(bookHash: string): Promise<void> {
  requireTauri()
  await invoke('delete_book', { bookHash })
}

export async function getProgress(bookHash: string): Promise<Progress | null> {
  requireTauri()
  return invoke<Progress | null>('get_progress', { bookHash })
}

export async function saveProgress(
  bookHash: string,
  cfi: string,
  fraction: number,
): Promise<void> {
  requireTauri()
  await invoke('save_progress', { bookHash, cfi, fraction })
}

export async function listAnnotations(bookHash: string): Promise<Annotation[]> {
  requireTauri()
  return invoke<Annotation[]>('list_annotations', { bookHash })
}

export async function saveAnnotation(annotation: AnnotationInput): Promise<Annotation> {
  requireTauri()
  return invoke<Annotation>('save_annotation', { annotation })
}

export async function deleteAnnotation(id: string): Promise<void> {
  requireTauri()
  await invoke('delete_annotation', { id })
}

export async function listCollections(): Promise<Collection[]> {
  requireTauri()
  return invoke<Collection[]>('list_collections')
}

export async function createCollection(name: string): Promise<Collection> {
  requireTauri()
  return invoke<Collection>('create_collection', { name })
}

export async function renameCollection(id: string, name: string): Promise<Collection> {
  requireTauri()
  return invoke<Collection>('rename_collection', { id, name })
}

export async function deleteCollection(id: string): Promise<void> {
  requireTauri()
  await invoke('delete_collection', { id })
}

export async function setBookCollection(
  bookHash: string,
  collectionId: string,
  added: boolean,
): Promise<void> {
  requireTauri()
  await invoke('set_book_collection', { bookHash, collectionId, added })
}

export async function listBookCollections(): Promise<BookCollection[]> {
  requireTauri()
  return invoke<BookCollection[]>('list_book_collections')
}

export async function syncStatus(): Promise<SyncStatus> {
  requireTauri()
  return invoke<SyncStatus>('sync_status')
}

export async function syncNow(): Promise<SyncRun> {
  requireTauri()
  return invoke<SyncRun>('sync_now')
}

export async function googleOAuthStart(): Promise<void> {
  requireTauri()
  await invoke('google_oauth_start')
}

export async function googleOAuthDisconnect(): Promise<void> {
  requireTauri()
  await invoke('google_oauth_disconnect')
}

export async function deviceId(): Promise<string> {
  requireTauri()
  return invoke<string>('device_id')
}

export async function takePendingOpen(): Promise<string[]> {
  requireTauri()
  return invoke<string[]>('take_pending_open')
}

export async function setWindowTheme(
  theme: 'light' | 'dark' | 'system' | 'sepia',
  caption: string,
  text: string,
): Promise<void> {
  if (!isTauri()) return
  await invoke('set_window_theme', { theme, caption, text })
}
