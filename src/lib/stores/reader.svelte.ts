import * as api from '../ipc/commands'
import type { Annotation, AnnotationInput, Book } from '../ipc/commands'

export type ReaderTheme = 'light' | 'dark' | 'sepia'
export type SidebarTab = 'toc' | 'notes' | 'search' | null

export interface TocItem {
  label: string
  href: string
  subitems?: TocItem[] | null
}

export interface TextSelection {
  text: string
  cfi: string
  x: number
  y: number
}

const THEME_KEY = 'foliate.theme'
const FONT_KEY = 'foliate.fontSize'
const LINE_KEY = 'foliate.lineHeight'

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

class ReaderStore {
  activeBook = $state<Book | null>(null)
  cfi = $state<string | null>(null)
  fraction = $state(0)
  toc = $state<TocItem[]>([])
  annotations = $state<Annotation[]>([])
  loading = $state(false)
  error = $state<string | null>(null)
  theme = $state<ReaderTheme>('light')
  fontSize = $state(1)
  lineHeight = $state(1.6)
  sidebarTab = $state<SidebarTab>(null)
  typographyOpen = $state(false)
  ttsActive = $state(false)
  selection = $state<TextSelection | null>(null)
  private saveTimer: number | undefined

  constructor() {
    if (typeof localStorage !== 'undefined') {
      const theme = localStorage.getItem(THEME_KEY)
      if (theme === 'light' || theme === 'dark' || theme === 'sepia') this.theme = theme
      const font = Number(localStorage.getItem(FONT_KEY))
      if (Number.isFinite(font) && font >= 0.8 && font <= 2) this.fontSize = font
      const line = Number(localStorage.getItem(LINE_KEY))
      if (Number.isFinite(line) && line >= 1.2 && line <= 2.4) this.lineHeight = line
    }
    this.applyTheme()
  }

  async open(book: Book): Promise<void> {
    this.loading = true
    this.error = null
    this.sidebarTab = null
    this.typographyOpen = false
    this.selection = null
    this.ttsActive = false
    this.cfi = book.progressCfi
    this.fraction = book.progressFraction
    this.annotations = []
    this.toc = []

    try {
      const [progress, annotations] = await Promise.all([
        api.getProgress(book.bookHash),
        api.listAnnotations(book.bookHash),
      ])
      if (progress) {
        this.cfi = progress.cfi
        this.fraction = progress.fraction
      }
      this.annotations = annotations
    } catch (error) {
      this.error = message(error)
    } finally {
      this.activeBook = book
      this.loading = false
    }
  }

  close(): void {
    void this.flushProgress()
    this.activeBook = null
    this.cfi = null
    this.fraction = 0
    this.toc = []
    this.annotations = []
    this.sidebarTab = null
    this.typographyOpen = false
    this.selection = null
    this.ttsActive = false
  }

  onRelocate(detail: { cfi?: string; fraction?: number }): void {
    if (!this.activeBook) return
    if (typeof detail.cfi === 'string') this.cfi = detail.cfi
    if (typeof detail.fraction === 'number') this.fraction = detail.fraction
    this.selection = null
    this.scheduleProgressSave()
  }

  private scheduleProgressSave(): void {
    if (this.saveTimer !== undefined) window.clearTimeout(this.saveTimer)
    this.saveTimer = window.setTimeout(() => {
      this.saveTimer = undefined
      void this.flushProgress()
    }, 1200)
  }

  async flushProgress(): Promise<void> {
    const book = this.activeBook
    if (!book || !this.cfi) return
    try {
      await api.saveProgress(book.bookHash, this.cfi, this.fraction)
      book.progressCfi = this.cfi
      book.progressFraction = this.fraction
    } catch {
      // Progress is best-effort; the next relocation retries.
    }
  }

  async addAnnotation(input: Omit<AnnotationInput, 'bookHash'>): Promise<void> {
    if (!this.activeBook) return
    try {
      const saved = await api.saveAnnotation({
        ...input,
        bookHash: this.activeBook.bookHash,
      })
      this.annotations = [...this.annotations, saved]
      this.selection = null
    } catch (error) {
      this.error = message(error)
    }
  }

  async removeAnnotation(id: string): Promise<void> {
    try {
      await api.deleteAnnotation(id)
      this.annotations = this.annotations.filter((annotation) => annotation.id !== id)
    } catch (error) {
      this.error = message(error)
    }
  }

  toggleSidebar(tab: Exclude<SidebarTab, null>): void {
    this.sidebarTab = this.sidebarTab === tab ? null : tab
    this.typographyOpen = false
  }

  toggleTypography(): void {
    this.typographyOpen = !this.typographyOpen
    if (this.typographyOpen) this.sidebarTab = null
  }

  setTheme(theme: ReaderTheme): void {
    this.theme = theme
    this.applyTheme()
  }

  cycleTheme(): void {
    const next: Record<ReaderTheme, ReaderTheme> = {
      light: 'sepia',
      sepia: 'dark',
      dark: 'light',
    }
    this.setTheme(next[this.theme])
  }

  setFontSize(size: number): void {
    this.fontSize = Math.min(2, Math.max(0.8, size))
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(FONT_KEY, String(this.fontSize))
    }
  }

  setLineHeight(height: number): void {
    this.lineHeight = Math.min(2.4, Math.max(1.2, height))
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(LINE_KEY, String(this.lineHeight))
    }
  }

  setSelection(selection: TextSelection | null): void {
    this.selection = selection
  }

  requestNavigation(target: string): void {
    if (typeof window === 'undefined' || !target) return
    window.dispatchEvent(new CustomEvent('foliate-go-to', { detail: target }))
  }

  private applyTheme(): void {
    if (typeof document !== 'undefined') {
      document.documentElement.dataset.theme = this.theme
    }
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(THEME_KEY, this.theme)
    }
  }
}

export const reader = new ReaderStore()
