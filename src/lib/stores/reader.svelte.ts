import * as api from '../ipc/commands'
import type { Annotation, AnnotationInput, Book } from '../ipc/commands'

export type ReaderTheme = 'system' | 'light' | 'dark' | 'sepia'
export type ReaderFont = 'serif' | 'sans' | 'mono'
export type SidebarTab = 'toc' | 'notes' | 'bookmarks' | 'search'
export type PageLayout = 'scroll' | 'single' | 'spread'

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

const THEME_KEY = 'wolidate.theme'
const FONT_KEY = 'wolidate.fontSize'
const LINE_KEY = 'wolidate.lineHeight'
const FAMILY_KEY = 'wolidate.fontFamily'
const LAYOUT_KEY = 'wolidate.pageLayout'

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
  theme = $state<ReaderTheme>('system')
  resolvedTheme = $state<'light' | 'dark' | 'sepia'>('light')
  fontSize = $state(1)
  lineHeight = $state(1.6)
  fontFamily = $state<ReaderFont>('serif')
  pageLayout = $state<PageLayout>('scroll')
  sidebarTab = $state<SidebarTab>('toc')
  sidebarOpen = $state(false)
  sidebarPinned = $state(true)
  detailsOpen = $state(false)
  typographyOpen = $state(false)
  ttsActive = $state(false)
  selection = $state<TextSelection | null>(null)
  location = $state<{ current: number; total: number } | null>(null)
  private saveTimer: number | undefined

  constructor() {
    if (typeof localStorage !== 'undefined') {
      const theme = localStorage.getItem(THEME_KEY)
      if (
        theme === 'system' ||
        theme === 'light' ||
        theme === 'dark' ||
        theme === 'sepia'
      ) {
        this.theme = theme
      }
      const font = Number(localStorage.getItem(FONT_KEY))
      if (Number.isFinite(font) && font >= 0.8 && font <= 2) this.fontSize = font
      const line = Number(localStorage.getItem(LINE_KEY))
      if (Number.isFinite(line) && line >= 1.2 && line <= 2.4) this.lineHeight = line
      const family = localStorage.getItem(FAMILY_KEY)
      if (family === 'serif' || family === 'sans' || family === 'mono') {
        this.fontFamily = family
      }
      const layout = localStorage.getItem(LAYOUT_KEY)
      if (layout === 'scroll' || layout === 'single' || layout === 'spread') {
        this.pageLayout = layout
      }
    }
    this.applyTheme()
    if (typeof window !== 'undefined') {
      window
        .matchMedia('(prefers-color-scheme: dark)')
        .addEventListener('change', () => {
          if (this.theme === 'system') this.applyTheme()
        })
    }
  }

  async open(book: Book): Promise<void> {
    this.loading = true
    this.error = null
    this.sidebarTab = 'toc'
    this.sidebarOpen =
      typeof window !== 'undefined' &&
      window.matchMedia('(min-width: 900px)').matches
    this.detailsOpen = false
    this.typographyOpen = false
    this.selection = null
    this.ttsActive = false
    this.location = null
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
    this.sidebarOpen = false
    this.detailsOpen = false
    this.typographyOpen = false
    this.selection = null
    this.ttsActive = false
    this.location = null
  }

  onRelocate(detail: {
    cfi?: string
    fraction?: number
    location?: { current: number; total: number }
  }): void {
    if (!this.activeBook) return
    if (typeof detail.cfi === 'string') this.cfi = detail.cfi
    if (typeof detail.fraction === 'number') this.fraction = detail.fraction
    if (detail.location) this.location = detail.location
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

  toggleSidebar(tab: SidebarTab): void {
    if (this.sidebarOpen && this.sidebarTab === tab) {
      this.sidebarOpen = false
    } else {
      this.sidebarTab = tab
      this.sidebarOpen = true
    }
    this.typographyOpen = false
  }

  setSidebarTab(tab: SidebarTab): void {
    this.sidebarTab = tab
    this.sidebarOpen = true
    this.typographyOpen = false
  }

  toggleTypography(): void {
    this.typographyOpen = !this.typographyOpen
  }

  setTheme(theme: ReaderTheme): void {
    this.theme = theme
    this.applyTheme()
  }

  setPageLayout(layout: PageLayout): void {
    this.pageLayout = layout
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(LAYOUT_KEY, layout)
    }
  }

  cycleTheme(): void {
    const next: Record<ReaderTheme, ReaderTheme> = {
      system: 'light',
      light: 'sepia',
      sepia: 'dark',
      dark: 'system',
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

  setFontFamily(family: ReaderFont): void {
    this.fontFamily = family
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(FAMILY_KEY, family)
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
    const systemDark =
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-color-scheme: dark)').matches
    const resolved: 'light' | 'dark' | 'sepia' =
      this.theme === 'system'
        ? systemDark
          ? 'dark'
          : 'light'
        : this.theme
    this.resolvedTheme = resolved
    if (typeof document !== 'undefined') {
      document.documentElement.dataset.theme = resolved
    }
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(THEME_KEY, this.theme)
    }
    if (typeof document !== 'undefined') {
      const style = getComputedStyle(document.documentElement)
      void api
        .setWindowTheme(
          this.theme,
          style.getPropertyValue('--bg-elevated').trim(),
          style.getPropertyValue('--text').trim(),
        )
        .catch(() => undefined)
    }
  }
}

export const reader = new ReaderStore()
