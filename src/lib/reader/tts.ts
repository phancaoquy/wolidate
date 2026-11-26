export interface TtsHandlers {
  onStart?: () => void
  onBoundary?: (charIndex: number, charLength: number) => void
  onEnd?: () => void
  onError?: (message: string) => void
}

/**
 * Thin Web Speech API wrapper. foliate-js can highlight a range while a
 * sentence is spoken; v0.1 exposes the boundary event so ReaderView can add
 * that later without changing the store.
 */
export class TtsController {
  readonly supported =
    typeof window !== 'undefined' && 'speechSynthesis' in window

  private rate = 1
  private pitch = 1

  get speaking(): boolean {
    return this.supported && window.speechSynthesis.speaking
  }

  setRate(rate: number): void {
    this.rate = Math.min(2, Math.max(0.5, rate))
  }

  setPitch(pitch: number): void {
    this.pitch = Math.min(2, Math.max(0.5, pitch))
  }

  speak(text: string, handlers: TtsHandlers = {}): void {
    if (!this.supported) {
      handlers.onError?.('This WebView does not expose speech synthesis.')
      return
    }
    this.stop()
    const cleaned = text.replace(/\s+/g, ' ').trim()
    if (!cleaned) return

    const utterance = new SpeechSynthesisUtterance(cleaned)
    utterance.rate = this.rate
    utterance.pitch = this.pitch
    utterance.lang = document.documentElement.lang || 'en'
    utterance.onstart = () => handlers.onStart?.()
    utterance.onboundary = (event) =>
      handlers.onBoundary?.(event.charIndex, event.charLength ?? 0)
    utterance.onend = () => {
      handlers.onEnd?.()
    }
    utterance.onerror = (event) => {
      handlers.onError?.(event.error || 'speech synthesis failed')
    }
    window.speechSynthesis.speak(utterance)
  }

  stop(): void {
    if (!this.supported) return
    window.speechSynthesis.cancel()
  }
}
