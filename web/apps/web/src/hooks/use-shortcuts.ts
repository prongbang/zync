// P2.3 — global keyboard shortcut map (Fork-like). One window keydown listener,
// installed once; handlers are read through a ref so the effect never re-binds.
// SHORTCUTS is the single source of truth shared with the cheat-sheet dialog.

import { useEffect, useRef } from "react"

export type ShortcutHandlers = {
  /** Cmd/Ctrl+P (or Cmd/Ctrl+K) — toggle the command palette. */
  onOpenPalette: () => void
  /** Cmd/Ctrl+Enter — commit (fires even from the composer input). */
  onCommit: () => void
  /** Cmd/Ctrl+Shift+F — focus the commit search field. */
  onFocusSearch: () => void
  /** Cmd/Ctrl+R — refresh the workspace. */
  onRefresh: () => void
  /** "?" — open the keyboard-shortcuts cheat sheet. */
  onShowShortcuts: () => void
  /** Whether a repository is open. When false, the repo-only ⌘R shortcut is not
   * intercepted so the browser's native reload still works (onRefresh would
   * otherwise no-op and swallow reload — e.g. on the empty-state screen). */
  hasRepo: boolean
}

// `mod` is rendered as ⌘ on macOS and Ctrl elsewhere by the cheat sheet.
export type ShortcutDef = { keys: string[]; description: string }

export const SHORTCUTS: ShortcutDef[] = [
  { keys: ["mod", "P"], description: "Open command palette" },
  { keys: ["mod", "K"], description: "Open command palette (alias)" },
  { keys: ["mod", "Enter"], description: "Commit staged changes" },
  { keys: ["mod", "Shift", "F"], description: "Search commits" },
  { keys: ["mod", "R"], description: "Refresh workspace" },
  { keys: ["?"], description: "Show keyboard shortcuts" },
  { keys: ["Esc"], description: "Close palette or dialog" },
]

export const IS_MAC =
  typeof navigator !== "undefined" && /mac/i.test(navigator.platform)

/** Human label for a single key token, platform-aware for `mod`. */
export function formatKey(key: string): string {
  if (key === "mod") return IS_MAC ? "⌘" : "Ctrl"
  if (key === "Shift") return IS_MAC ? "⇧" : "Shift"
  if (key === "Enter") return IS_MAC ? "↩" : "Enter"
  return key
}

// A control that owns text entry — most shortcuts must not steal keys from it.
function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  const tag = target.tagName
  return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable
}

export function useShortcuts(handlers: ShortcutHandlers): void {
  const ref = useRef(handlers)
  ref.current = handlers

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const mod = event.metaKey || event.ctrlKey
      const key = event.key
      const h = ref.current

      // Palette: Cmd/Ctrl+P or Cmd/Ctrl+K. Works from anywhere (modifier-gated),
      // so it's safe even while a field is focused.
      if (mod && !event.shiftKey && !event.altKey && (key === "p" || key === "k")) {
        event.preventDefault()
        h.onOpenPalette()
        return
      }
      // Focus commit search: Cmd/Ctrl+Shift+F.
      if (mod && event.shiftKey && (key === "f" || key === "F")) {
        event.preventDefault()
        h.onFocusSearch()
        return
      }
      // Commit: Cmd/Ctrl+Enter. Intentionally allowed while typing in the
      // commit composer — that's the whole point.
      if (mod && key === "Enter") {
        event.preventDefault()
        h.onCommit()
        return
      }
      // Refresh: Cmd/Ctrl+R (no Shift, to leave hard-reload alone). Only
      // intercept when a repo is open — otherwise onRefresh no-ops and we'd be
      // swallowing the browser's native reload for nothing (e.g. empty state).
      if (mod && !event.shiftKey && (key === "r" || key === "R")) {
        if (!h.hasRepo) return
        event.preventDefault()
        h.onRefresh()
        return
      }
      // Cheat sheet: "?" — only when not typing into a field.
      if (!mod && key === "?" && !isTypingTarget(event.target)) {
        event.preventDefault()
        h.onShowShortcuts()
        return
      }
    }

    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [])
}
