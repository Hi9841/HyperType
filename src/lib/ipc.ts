import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { open, save } from "@tauri-apps/plugin-dialog";

export type TriggerKind = "text" | "shortcut";

export type InsertMode = "auto" | "paste" | "type";
export type PasteCombo = "ctrl_v" | "shift_insert" | "ctrl_shift_v";

export const MAX_TEXT_TRIGGER_CHARS = 64;
const MAX_EXPANSION_CHARS = 1_000_000;

function validateSnippet(trigger: string, expansion: string, kind: TriggerKind): void {
  if (!trigger || !expansion) throw "Both trigger and expansion are required.";
  if ([...expansion].length > MAX_EXPANSION_CHARS) throw "Expansion is too long.";
  if (kind === "text") {
    if ([...trigger].length > MAX_TEXT_TRIGGER_CHARS) throw "Text trigger is too long.";
    if (/\p{Cc}/u.test(trigger)) throw "Text triggers cannot contain control characters.";
  }
}

export interface SnippetView {
  trigger: string;
  expansion: string;
  kind: TriggerKind;
}

export interface Status {
  enabled: boolean;
  count: number;
  version: string;
  insert_mode: InsertMode;
  wpm: number;
  paste_combo: PasteCombo;
  restore_delay_ms: number;
  auto_paste_words: number;
  hook_active: boolean;
  hook_events: number;
  hook_healthy: boolean;
  is_elevated: boolean;
}

export interface ImportSummary {
  imported: number;
  skipped: number;
}

interface Api {
  getStatus: () => Promise<Status>;
  getSnippets: () => Promise<SnippetView[]>;
  addSnippet: (trigger: string, expansion: string, kind: TriggerKind) => Promise<void>;
  editSnippet: (
    oldTrigger: string,
    trigger: string,
    expansion: string,
    kind: TriggerKind,
  ) => Promise<void>;
  removeSnippet: (trigger: string) => Promise<void>;
  exportSnippets: () => Promise<string | null>;
  importSnippets: () => Promise<ImportSummary | null>;
  reorderSnippets: (order: string[]) => Promise<void>;
  toggleEnabled: () => Promise<boolean>;
  setInsertMode: (mode: InsertMode) => Promise<void>;
  setWpm: (wpm: number) => Promise<void>;
  setPasteCombo: (combo: PasteCombo) => Promise<void>;
  setRestoreDelay: (delayMs: number) => Promise<void>;
  setAutoPasteWords: (words: number) => Promise<void>;
  getAutostart: () => Promise<boolean>;
  setAutostart: (on: boolean) => Promise<void>;
  reinstallHook: () => Promise<boolean>;
  restartElevated: () => Promise<void>;
  quit: () => Promise<void>;
}

const inTauri = "__TAURI_INTERNALS__" in window;

// Thin typed wrapper over the Rust command surface. The frontend never
// touches the keyboard/engine path or the global-shortcut plugin directly;
// it only manages snippets and state, and Rust does all OS-level work.
// Autostart goes through the autostart plugin's own bindings (registers a
// per-user run entry launching `hypertype.exe --minimized`).
const tauriApi: Api = {
  getStatus: () => invoke<Status>("get_status"),
  getSnippets: () => invoke<SnippetView[]>("get_snippets"),
  addSnippet: (trigger, expansion, kind) =>
    invoke<void>("add_snippet", { trigger, expansion, kind }),
  editSnippet: (oldTrigger, trigger, expansion, kind) =>
    invoke<void>("edit_snippet", { oldTrigger, trigger, expansion, kind }),
  removeSnippet: (trigger) => invoke<void>("remove_snippet", { trigger }),
  exportSnippets: async () => {
    const path = await save({
      defaultPath: "HyperType-snippets.json",
      filters: [{ name: "HyperType snippets", extensions: ["json"] }],
    });
    if (!path) return null;
    await invoke<void>("export_snippets", { path });
    return path;
  },
  importSnippets: async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "HyperType snippets", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return null;
    return invoke<ImportSummary>("import_snippets", { path });
  },
  reorderSnippets: (order) => invoke<void>("reorder_snippets", { order }),
  toggleEnabled: () => invoke<boolean>("toggle_enabled"),
  setInsertMode: (mode) => invoke<void>("set_insert_mode", { mode }),
  setWpm: (wpm) => invoke<void>("set_wpm", { wpm }),
  setPasteCombo: (combo) => invoke<void>("set_paste_combo", { combo }),
  setRestoreDelay: (delayMs) => invoke<void>("set_restore_delay_ms", { delayMs }),
  setAutoPasteWords: (words) => invoke<void>("set_auto_paste_words", { words }),
  getAutostart: () => isEnabled(),
  setAutostart: (on) => (on ? enable() : disable()),
  reinstallHook: () => invoke<boolean>("reinstall_hook"),
  restartElevated: () => invoke<void>("restart_elevated"),
  quit: () => invoke<void>("quit_app"),
};

// In-memory stand-in for `bun run dev` in a plain browser, where no Tauri
// backend answers invoke(). Seeded like storage::load_or_default so the UI
// can be exercised and styled without the Rust core.
function browserMock(): Api {
  let enabled = true;
  let autostart = false;
  let insertMode: InsertMode = "auto";
  let wpm = 600;
  let pasteCombo: PasteCombo = "ctrl_v";
  let restoreDelayMs = 5000;
  let autoPasteWords = 15;
  const store = new Map<string, { expansion: string; kind: TriggerKind }>([
    ["gm", { expansion: "Good morning", kind: "text" }],
    ["addr", { expansion: "123 Main Street, Springfield", kind: "text" }],
    ["sig", { expansion: "Best regards,\nYour Name", kind: "text" }],
    ["brb", { expansion: "Be right back", kind: "text" }],
    ["omw", { expansion: "On my way", kind: "text" }],
  ]);
  return {
    getStatus: async () => ({
      enabled,
      count: store.size,
      version: "1.1.5",
      insert_mode: insertMode,
      wpm,
      paste_combo: pasteCombo,
      restore_delay_ms: restoreDelayMs,
      auto_paste_words: autoPasteWords,
      hook_active: true,
      hook_events: 42,
      hook_healthy: true,
      is_elevated: false,
    }),
    getSnippets: async () =>
      [...store.entries()].map(([trigger, entry]) => ({ trigger, ...entry })),
    addSnippet: async (trigger, expansion, kind) => {
      const t = trigger.trim();
      validateSnippet(t, expansion, kind);
      if (store.has(t)) throw "A snippet with that trigger already exists.";
      store.set(t, { expansion, kind });
    },
    editSnippet: async (oldTrigger, trigger, expansion, kind) => {
      const t = trigger.trim();
      if (!store.has(oldTrigger)) throw "Snippet not found.";
      validateSnippet(t, expansion, kind);
      if (oldTrigger !== t && store.has(t)) {
        throw "A snippet with that trigger already exists.";
      }
      const next = new Map<string, { expansion: string; kind: TriggerKind }>();
      for (const [key, entry] of store) {
        next.set(key === oldTrigger ? t : key, key === oldTrigger ? { expansion, kind } : entry);
      }
      store.clear();
      for (const [key, entry] of next) store.set(key, entry);
    },
    removeSnippet: async (trigger) => {
      store.delete(trigger);
    },
    exportSnippets: async () => {
      const entries = [...store.entries()].map(([trigger, entry]) => ({
        trigger,
        expansion: entry.expansion,
        kind: entry.kind,
      }));
      const url = URL.createObjectURL(
        new Blob([JSON.stringify(entries, null, 2)], { type: "application/json" }),
      );
      const a = document.createElement("a");
      a.href = url;
      a.download = "HyperType-snippets.json";
      a.click();
      URL.revokeObjectURL(url);
      return "HyperType-snippets.json";
    },
    importSnippets: async () =>
      new Promise<ImportSummary | null>((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = "application/json,.json";
        input.onchange = async () => {
          const file = input.files?.[0];
          if (!file) return resolve(null);
          try {
            const parsed = JSON.parse(await file.text());
            const entries: SnippetView[] = Array.isArray(parsed)
              ? parsed
              : Object.entries(parsed).map(([trigger, expansion]) => ({
                  trigger,
                  expansion: String(expansion),
                  kind: "text" as TriggerKind,
                }));
            let imported = 0;
            let skipped = 0;
            for (const entry of entries) {
              const trigger = String(entry.trigger ?? "").trim();
              const expansion = String(entry.expansion ?? "");
              const kind = entry.kind === "shortcut" ? "shortcut" : "text";
              try {
                validateSnippet(trigger, expansion, kind);
              } catch {
                skipped += 1;
                continue;
              }
              store.set(trigger, { expansion, kind });
              imported += 1;
            }
            resolve({ imported, skipped });
          } catch {
            resolve({ imported: 0, skipped: 1 });
          }
        };
        input.click();
      }),
    reorderSnippets: async (order) => {
      const next = new Map<string, { expansion: string; kind: TriggerKind }>();
      for (const t of order) {
        const entry = store.get(t);
        if (entry) next.set(t, entry);
      }
      for (const [t, entry] of store) if (!next.has(t)) next.set(t, entry);
      store.clear();
      for (const [t, entry] of next) store.set(t, entry);
    },
    toggleEnabled: async () => (enabled = !enabled),
    setInsertMode: async (mode) => {
      insertMode = mode;
    },
    setWpm: async (value) => {
      wpm = value;
    },
    setPasteCombo: async (combo) => {
      pasteCombo = combo;
    },
    setRestoreDelay: async (delayMs) => {
      restoreDelayMs = Math.min(15000, Math.max(3000, delayMs));
    },
    setAutoPasteWords: async (words) => {
      autoPasteWords = Math.min(500, Math.max(1, words));
    },
    getAutostart: async () => autostart,
    setAutostart: async (on) => {
      autostart = on;
    },
    reinstallHook: async () => true,
    restartElevated: async () => {},
    quit: async () => {},
  };
}

export const api: Api = inTauri ? tauriApi : browserMock();

/** Frameless-window controls for the custom titlebar. No-ops in a browser. */
export const win = {
  minimize: () => (inTauri ? getCurrentWindow().minimize() : Promise.resolve()),
  close: () => (inTauri ? getCurrentWindow().close() : Promise.resolve()),
};

/** Fires when the engine is toggled outside the window (tray menu). */
export function onEnabledChanged(handler: (enabled: boolean) => void): Promise<() => void> {
  if (!inTauri) return Promise.resolve(() => {});
  return listen<boolean>("enabled-changed", (event) => handler(event.payload));
}
