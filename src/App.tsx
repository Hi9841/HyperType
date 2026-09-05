import { createMemo, createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import {
  api,
  MAX_TEXT_TRIGGER_CHARS,
  onEnabledChanged,
  win,
  type InsertMode,
  type PasteCombo,
  type SnippetView,
  type TriggerKind,
} from "./lib/ipc";
import { chordFromEvent, chordKeys } from "./lib/shortcut";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import {
  IconCheck,
  IconClose,
  IconCloseWindow,
  IconDocument,
  IconDownload,
  IconExport,
  IconGrip,
  IconImport,
  IconMinimize,
  IconPlus,
  IconRefresh,
  IconSearch,
  IconSettings,
} from "./components/Icons";
import logo from "./assets/logo.png";

const INSERT_MODES: InsertMode[] = ["auto", "paste", "type"];
const INSERT_LABEL: Record<InsertMode, string> = { auto: "Auto", paste: "Paste", type: "Type" };
function insertSub(mode: InsertMode, words: number): string {
  if (mode === "paste") return "Always pastes via clipboard";
  if (mode === "type") return "Always types at configured WPM";
  return `Types out ≤ ${words} words at set WPM; pastes longer text`;
}
const PASTE_COMBOS: PasteCombo[] = ["ctrl_v", "shift_insert", "ctrl_shift_v"];
const COMBO_LABEL: Record<PasteCombo, string> = {
  ctrl_v: "Ctrl+V",
  shift_insert: "Shift+Ins",
  ctrl_shift_v: "Ctrl+⇧+V",
};

export default function App() {
  const [status, { refetch: refetchStatus, mutate: mutateStatus }] =
    createResource(api.getStatus);
  const [snippets, { refetch: refetchSnippets, mutate: mutateSnippets }] =
    createResource(api.getSnippets);
  const [autostart, setAutostart] = createSignal(false);

  // Updater State
  const [updateInfo, setUpdateInfo] = createSignal<{ version: string; body?: string } | null>(null);
  const [updateChecking, setUpdateChecking] = createSignal(false);
  const [updateStatusText, setUpdateStatusText] = createSignal("");
  const [updateDownloading, setUpdateDownloading] = createSignal(false);
  const [updateProgress, setUpdateProgress] = createSignal(0);
  let availableUpdateObj: Update | null = null;

  async function checkForUpdate(manual = false) {
    if (updateChecking() || updateDownloading()) return;
    setUpdateChecking(true);
    if (manual) setUpdateStatusText("Checking for updates...");
    try {
      const update = await check({
        timeout: 15000,
        target: "windows-x86_64-nsis",
      });
      if (update?.available) {
        availableUpdateObj = update;
        setUpdateInfo({ version: update.version, body: update.body });
        setUpdateStatusText(`v${update.version} available`);
      } else {
        availableUpdateObj = null;
        setUpdateInfo(null);
        if (manual) {
          setUpdateStatusText("HyperType is up to date");
          setTimeout(() => setUpdateStatusText(""), 4000);
        }
      }
    } catch {
      if (manual) {
        setUpdateStatusText("Update check failed");
        setTimeout(() => setUpdateStatusText(""), 4000);
      }
    } finally {
      setUpdateChecking(false);
    }
  }

  async function installDownloadedUpdate() {
    if (!availableUpdateObj || updateDownloading()) return;
    setUpdateDownloading(true);
    setUpdateProgress(0);
    let downloaded = 0;
    let total = 0;
    try {
      await availableUpdateObj.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total > 0) setUpdateProgress(Math.round((downloaded / total) * 100));
        }
      });
      setUpdateStatusText("Update installed. Restarting...");
      try {
        await relaunch();
      } catch {
        setUpdateStatusText("Update installed. Restart HyperType.");
        setUpdateDownloading(false);
      }
    } catch (err) {
      console.error("[updater] downloadAndInstall error:", err);
      const msg = err instanceof Error ? err.message : String(err);
      setUpdateStatusText(`Update failed: ${msg.slice(0, 45)}`);
      setUpdateDownloading(false);
    }
  }

  // Search & Navigation
  const [search, setSearch] = createSignal("");
  const [selectedTrigger, setSelectedTrigger] = createSignal<string | null>(null);
  const [isCreatingNew, setIsCreatingNew] = createSignal(false);
  const [showSettings, setShowSettings] = createSignal(false);
  const [showTokensModal, setShowTokensModal] = createSignal(false);

  // Editor Form State
  const [mode, setMode] = createSignal<TriggerKind>("text");
  const [trigger, setTrigger] = createSignal("");
  const [expansion, setExpansion] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [recording, setRecording] = createSignal(false);
  const [formError, setFormError] = createSignal("");
  const [formSuccess, setFormSuccess] = createSignal("");

  // Transfer & Reorder
  const [transferBusy, setTransferBusy] = createSignal(false);
  const [transferMessage, setTransferMessage] = createSignal("");
  const [dragIdx, setDragIdx] = createSignal<number | null>(null);
  const [dragY, setDragY] = createSignal(0);
  const [dropIdx, setDropIdx] = createSignal(0);
  let rowHeight = 52;

  // Settings State
  const [wpmDrag, setWpmDrag] = createSignal<number | null>(null);
  const [restoreDrag, setRestoreDrag] = createSignal<number | null>(null);
  const [autoWordsDrag, setAutoWordsDrag] = createSignal<number | null>(null);
  const [repairingHook, setRepairingHook] = createSignal(false);
  const [hookMessage, setHookMessage] = createSignal("");

  async function handleRepairHook() {
    if (repairingHook()) return;
    setRepairingHook(true);
    setHookMessage("Reinstalling hook...");
    try {
      await api.reinstallHook();
      await refetchStatus();
      setHookMessage("Hook active & reconnected!");
      setTimeout(() => setHookMessage(""), 3500);
    } catch {
      setHookMessage("Failed to reinstall hook");
      setTimeout(() => setHookMessage(""), 3500);
    } finally {
      setRepairingHook(false);
    }
  }

  let triggerInput: HTMLInputElement | undefined;
  let expansionTextarea: HTMLTextAreaElement | undefined;
  let stopRecording: (() => void) | undefined;

  onMount(() => {
    api.getAutostart().then(setAutostart).catch(() => {});
    const unlisten = onEnabledChanged((enabled) => {
      const s = status();
      if (s) mutateStatus({ ...s, enabled });
    });
    onCleanup(() => unlisten.then((f) => f()));

    // Keep hook event counter fresh when Settings modal is open
    const interval = setInterval(() => {
      if (showSettings()) {
        refetchStatus();
      }
    }, 1500);
    onCleanup(() => clearInterval(interval));

    checkForUpdate(false);
  });

  // Filter snippets based on search
  const filteredSnippets = createMemo(() => {
    const list = snippets() ?? [];
    const q = search().trim().toLowerCase();
    if (!q) return list;
    return list.filter(
      (s) =>
        s.trigger.toLowerCase().includes(q) ||
        s.expansion.toLowerCase().includes(q),
    );
  });

  // Keep editor in sync when a snippet is clicked
  function selectSnippet(s: SnippetView) {
    stopRecording?.();
    setIsCreatingNew(false);
    setSelectedTrigger(s.trigger);
    setMode(s.kind);
    setTrigger(s.trigger);
    setExpansion(s.expansion);
    setFormError("");
    setFormSuccess("");
  }

  function startNewSnippet() {
    stopRecording?.();
    setIsCreatingNew(true);
    setSelectedTrigger(null);
    setMode("text");
    setTrigger("");
    setExpansion("");
    setFormError("");
    setFormSuccess("");
    queueMicrotask(() => triggerInput?.focus());
  }

  // Initial selection once snippets arrive
  onMount(() => {
    const check = setInterval(() => {
      const list = snippets();
      if (list && list.length > 0 && selectedTrigger() === null && !isCreatingNew()) {
        selectSnippet(list[0]);
        clearInterval(check);
      }
    }, 100);
    onCleanup(() => clearInterval(check));
  });

  const enabled = () => status()?.enabled ?? false;
  const canSave = () =>
    !busy() && trigger().trim().length > 0 && expansion().length > 0;

  // Live text metrics
  const linesCount = () => (expansion() ? expansion().split("\n").length : 0);
  const wordsCount = () => {
    const text = expansion().trim();
    return text ? text.split(/\s+/).length : 0;
  };
  const charsCount = () => expansion().length;

  onCleanup(() => {
    stopRecording?.();
  });

  async function toggle() {
    const before = status();
    if (before) mutateStatus({ ...before, enabled: !before.enabled });
    try {
      const now = await api.toggleEnabled();
      const s = status();
      if (s) mutateStatus({ ...s, enabled: now });
    } catch {
      refetchStatus();
    }
  }

  const insertMode = () => status()?.insert_mode;

  async function changeInsertMode(next: InsertMode) {
    const s = status();
    if (s) mutateStatus({ ...s, insert_mode: next });
    try {
      await api.setInsertMode(next);
    } catch {
      refetchStatus();
    }
  }

  async function changePasteCombo(next: PasteCombo) {
    const s = status();
    if (s) mutateStatus({ ...s, paste_combo: next });
    try {
      await api.setPasteCombo(next);
    } catch {
      refetchStatus();
    }
  }

  const wpm = () => wpmDrag() ?? status()?.wpm ?? 600;

  async function commitWpm(value: number) {
    setWpmDrag(null);
    const s = status();
    if (s) mutateStatus({ ...s, wpm: value });
    try {
      await api.setWpm(value);
    } catch {
      refetchStatus();
    }
  }

  const restoreMs = () => restoreDrag() ?? status()?.restore_delay_ms ?? 5000;

  async function commitRestoreDelay(value: number) {
    setRestoreDrag(null);
    const s = status();
    if (s) mutateStatus({ ...s, restore_delay_ms: value });
    try {
      await api.setRestoreDelay(value);
    } catch {
      refetchStatus();
    }
  }

  const autoWords = () => autoWordsDrag() ?? status()?.auto_paste_words ?? 15;

  async function commitAutoPasteWords(value: number) {
    setAutoWordsDrag(null);
    const s = status();
    if (s) mutateStatus({ ...s, auto_paste_words: value });
    try {
      await api.setAutoPasteWords(value);
    } catch {
      refetchStatus();
    }
  }

  function commitValueText(
    el: HTMLInputElement,
    min: number,
    max: number,
    current: number,
    commit: (v: number) => void,
  ) {
    const n = Math.round(Number(el.value.trim()));
    if (el.value.trim() === "" || !Number.isFinite(n)) {
      el.value = String(current);
      return;
    }
    const clamped = Math.min(max, Math.max(min, n));
    el.value = String(clamped);
    if (clamped !== current) commit(clamped);
  }

  function updateTextTrigger(input: HTMLInputElement) {
    const limited = [...input.value].slice(0, MAX_TEXT_TRIGGER_CHARS).join("");
    if (limited !== input.value) input.value = limited;
    setTrigger(limited);
  }

  async function toggleAutostart() {
    const next = !autostart();
    setAutostart(next);
    try {
      await api.setAutostart(next);
    } catch {
      setAutostart(!next);
    }
  }

  function switchMode(next: TriggerKind) {
    if (mode() === next) return;
    stopRecording?.();
    setMode(next);
    setTrigger("");
    setFormError("");
    if (next === "text") queueMicrotask(() => triggerInput?.focus());
  }

  function startShortcutRecording() {
    if (recording()) return;
    setRecording(true);
    setTrigger("");
    setFormError("");

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.code === "Escape") {
        stop();
        return;
      }
      const chord = chordFromEvent(e);
      if (chord) {
        setTrigger(chord);
        stop();
        expansionTextarea?.focus();
      }
    };
    const stop = () => {
      window.removeEventListener("keydown", onKeyDown, true);
      setRecording(false);
      stopRecording = undefined;
    };
    stopRecording = stop;
    window.addEventListener("keydown", onKeyDown, true);
  }

  function insertToken(token: string) {
    const ta = expansionTextarea;
    if (!ta) {
      setExpansion((prev) => prev + token);
      return;
    }
    const start = ta.selectionStart ?? ta.value.length;
    const end = ta.selectionEnd ?? ta.value.length;
    const val = ta.value;
    const next = val.substring(0, start) + token + val.substring(end);
    setExpansion(next);
    queueMicrotask(() => {
      ta.focus();
      ta.selectionStart = ta.selectionEnd = start + token.length;
    });
  }

  async function saveOrAdd() {
    const t = trigger().trim();
    const x = expansion();
    if (!t || !x || busy()) return;
    setBusy(true);
    setFormError("");
    setFormSuccess("");

    try {
      const currentSelected = selectedTrigger();
      if (isCreatingNew() || !currentSelected) {
        await api.addSnippet(t, x, mode());
        setFormSuccess("Snippet created!");
      } else {
        await api.editSnippet(currentSelected, t, x, mode());
        setFormSuccess("Saved!");
      }
      setSelectedTrigger(t);
      setIsCreatingNew(false);
      await Promise.all([refetchSnippets(), refetchStatus()]);
      setTimeout(() => setFormSuccess(""), 3000);
    } catch (err) {
      setFormError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function deleteSnippet(t: string, e?: Event) {
    e?.stopPropagation();
    if (!confirm(`Delete snippet "${t}"?`)) return;
    try {
      await api.removeSnippet(t);
      const list = (await refetchSnippets()) ?? [];
      await refetchStatus();
      if (selectedTrigger() === t) {
        if (list.length > 0) {
          selectSnippet(list[0]);
        } else {
          startNewSnippet();
        }
      }
    } catch (err) {
      setFormError(String(err));
    }
  }

  async function applyReorder(from: number, to: number) {
    const list = snippets();
    if (!list || from === to) return;
    const next = [...list];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    mutateSnippets(next);
    try {
      await api.reorderSnippets(next.map((s) => s.trigger));
    } catch {
      refetchSnippets();
    }
  }

  function startDrag(e: PointerEvent & { currentTarget: HTMLElement }, index: number) {
    if (e.button !== 0 || dragIdx() !== null) return;
    e.preventDefault();
    rowHeight = (e.currentTarget.closest("div.studio-item") as HTMLElement)?.offsetHeight ?? 52;
    const startY = e.clientY;
    const count = snippets()?.length ?? 0;
    setDragIdx(index);
    setDropIdx(index);
    setDragY(0);

    const onMove = (ev: PointerEvent) => {
      const dy = ev.clientY - startY;
      setDragY(dy);
      setDropIdx(Math.min(count - 1, Math.max(0, index + Math.round(dy / rowHeight))));
    };
    const finish = (commit: boolean) => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
      const to = dropIdx();
      setDragIdx(null);
      setDragY(0);
      if (commit) applyReorder(index, to);
    };
    const onUp = () => finish(true);
    const onCancel = () => finish(false);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
  }

  function gripKeys(e: KeyboardEvent, index: number) {
    const count = snippets()?.length ?? 0;
    if (e.key === "ArrowUp" && index > 0) {
      e.preventDefault();
      applyReorder(index, index - 1);
    } else if (e.key === "ArrowDown" && index < count - 1) {
      e.preventDefault();
      applyReorder(index, index + 1);
    }
  }

  function rowShift(index: number): string | undefined {
    const from = dragIdx();
    if (from === null) return undefined;
    if (index === from) return `transform: translateY(${dragY()}px); z-index: 10;`;
    const to = dropIdx();
    if (from < to && index > from && index <= to) return `transform: translateY(-${rowHeight}px)`;
    if (from > to && index >= to && index < from) return `transform: translateY(${rowHeight}px)`;
    return undefined;
  }

  async function exportSnippets() {
    if (transferBusy()) return;
    setTransferBusy(true);
    setTransferMessage("");
    try {
      const path = await api.exportSnippets();
      if (path) setTransferMessage("Exported to JSON");
      setTimeout(() => setTransferMessage(""), 3000);
    } catch (err) {
      setTransferMessage(String(err));
    } finally {
      setTransferBusy(false);
    }
  }

  async function importSnippets() {
    if (transferBusy()) return;
    setTransferBusy(true);
    setTransferMessage("");
    try {
      const result = await api.importSnippets();
      if (result) {
        setTransferMessage(
          result.skipped > 0
            ? `Imported ${result.imported}, skipped ${result.skipped}`
            : `Imported ${result.imported}`,
        );
        await Promise.all([refetchSnippets(), refetchStatus()]);
        setTimeout(() => setTransferMessage(""), 3000);
      }
    } catch (err) {
      setTransferMessage(String(err));
    } finally {
      setTransferBusy(false);
    }
  }

  return (
    <div class="studio-app">
      {/* Studio Frameless Titlebar */}
      <header class="studio-titlebar" data-tauri-drag-region>
        <div class="studio-brand" data-tauri-drag-region>
          <div class="brand-icon-wrap" title={enabled() ? "Engine Active (click to pause)" : "Engine Paused (click to resume)"} onClick={toggle}>
            <img
              class="brand-mark"
              classList={{ paused: !enabled() }}
              src={logo}
              alt=""
              width="18"
              height="18"
            />
            <span class="brand-status-dot" classList={{ active: enabled() }} />
          </div>
          <span class="brand-name" data-tauri-drag-region>
            HyperType <span class="studio-pill">Studio</span>
          </span>
        </div>

        <div class="studio-win-controls">
          <Show when={updateInfo()}>
            <button
              class="update-available-pill"
              onClick={() => setShowSettings(true)}
              title={`HyperType v${updateInfo()!.version} available. Click to install.`}
            >
              <IconDownload /> Update v{updateInfo()!.version}
            </button>
          </Show>
          <button
            class="settings-icon-btn"
            title="Settings & Preferences"
            onClick={() => setShowSettings(true)}
          >
            <IconSettings />
          </button>
          <button class="winbtn" aria-label="Minimize" onClick={() => win.minimize()}>
            <IconMinimize />
          </button>
          <button class="winbtn close" aria-label="Close" onClick={() => win.close()}>
            <IconCloseWindow />
          </button>
        </div>
      </header>

      {/* Main Studio Split-Pane Workspace */}
      <main class="studio-workspace">
        {/* Left Pane: Searchable Snippet Library */}
        <aside class="studio-sidebar">
          <div class="sidebar-top-section">
            <div class="search-bar-wrap">
              <span class="search-icon">
                <IconSearch />
              </span>
              <input
                type="text"
                class="studio-search-input"
                placeholder="Search triggers or content..."
                value={search()}
                onInput={(e) => setSearch(e.currentTarget.value)}
              />
              <Show when={search()}>
                <button class="search-clear-btn" onClick={() => setSearch("")} title="Clear search">
                  <IconClose />
                </button>
              </Show>
            </div>

            <div class="sidebar-action-bar">
              <button class="new-snippet-btn" onClick={startNewSnippet}>
                <IconPlus /> New Snippet
              </button>
              <div class="library-io-btns">
                <button
                  type="button"
                  title="Export snippets to JSON"
                  onClick={exportSnippets}
                  disabled={transferBusy()}
                >
                  <IconExport /> Export
                </button>
                <button
                  type="button"
                  title="Import snippets from JSON"
                  onClick={importSnippets}
                  disabled={transferBusy()}
                >
                  <IconImport /> Import
                </button>
              </div>
            </div>

            <Show when={transferMessage()}>
              <div class="transfer-toast">{transferMessage()}</div>
            </Show>
          </div>

          <div class="sidebar-list-header">
            <span class="list-title">SNIPPETS</span>
            <span class="list-count">{filteredSnippets().length} total</span>
          </div>

          <div class="studio-snippets-list" classList={{ reordering: dragIdx() !== null }}>
            <Show
              when={filteredSnippets().length > 0}
              fallback={
                <div class="sidebar-empty">
                  <span class="empty-icon">
                    <IconDocument />
                  </span>
                  <p class="empty-text">
                    {search() ? "No matching snippets found" : "No snippets in library"}
                  </p>
                  <button class="btn-create-first" onClick={startNewSnippet}>
                    <IconPlus /> Create Snippet
                  </button>
                </div>
              }
            >
              <For each={filteredSnippets()}>
                {(s, i) => {
                  const isSelected = () => !isCreatingNew() && selectedTrigger() === s.trigger;
                  const lines = s.expansion.split("\n").length;
                  const chars = s.expansion.length;
                  return (
                    <div
                      class="studio-item"
                      classList={{
                        active: isSelected(),
                        dragging: dragIdx() === i(),
                      }}
                      style={rowShift(i())}
                      onClick={() => selectSnippet(s)}
                    >
                      <button
                        class="drag-grip"
                        aria-label={`Reorder ${s.trigger}`}
                        onPointerDown={(e) => startDrag(e, i())}
                        onKeyDown={(e) => gripKeys(e, i())}
                        onClick={(e) => e.stopPropagation()}
                      >
                        <IconGrip />
                      </button>

                      <div class="item-content-body">
                        <div class="item-header-row">
                          <Show
                            when={s.kind === "shortcut"}
                            fallback={<kbd class="trigger-keycap">{s.trigger}</kbd>}
                          >
                            <span class="chord-badge">
                              <For each={chordKeys(s.trigger)}>
                                {(key) => <kbd class="chord-key">{key}</kbd>}
                              </For>
                            </span>
                          </Show>

                          <span class="item-stats-badge">
                            {lines > 1 ? `${lines} lines` : `${chars} chars`}
                          </span>
                        </div>

                        <div class="item-excerpt" title={s.expansion}>
                          {s.expansion.replace(/\n/g, " ↵ ")}
                        </div>
                      </div>

                      <button
                        class="item-delete-btn"
                        title="Delete snippet"
                        onClick={(e) => deleteSnippet(s.trigger, e)}
                      >
                        <IconClose />
                      </button>
                    </div>
                  );
                }}
              </For>
            </Show>
          </div>
        </aside>

        {/* Right Pane: Dedicated Giant Editor Canvas */}
        <section class="studio-canvas">
          <div class="canvas-header">
            <div class="canvas-header-title">
              <h2>{isCreatingNew() ? "New Snippet" : `Editing: ${trigger() || "Snippet"}`}</h2>
              <span class="canvas-sub-hint">
                {mode() === "text"
                  ? "Triggers expand automatically at word boundaries"
                  : "Registered as global Windows hotkey chord"}
              </span>
            </div>

            <div class="seg seg-kind" role="group" aria-label="Trigger type" data-mode={mode()}>
              <span class="seg-thumb" aria-hidden="true" />
              <button
                type="button"
                classList={{ active: mode() === "text" }}
                onClick={() => switchMode("text")}
              >
                Text
              </button>
              <button
                type="button"
                classList={{ active: mode() === "shortcut" }}
                onClick={() => switchMode("shortcut")}
              >
                Shortcut
              </button>
            </div>
          </div>

          <div class="trigger-config-row">
            <div class="trigger-input-wrapper">
              <label class="field-label">
                {mode() === "text" ? "ABBREVIATION / TRIGGER" : "SHORTCUT CHORD"}
              </label>

              <Show
                when={mode() === "text"}
                fallback={
                  <button
                    type="button"
                    class="studio-field recorder-btn"
                    classList={{ listening: recording() }}
                    onClick={startShortcutRecording}
                  >
                    <Show
                      when={!recording() && trigger()}
                      fallback={
                        <span class="recorder-hint-text">
                          {recording() ? "Press keys on keyboard… Esc cancels" : "Click to Record Shortcut Chord"}
                        </span>
                      }
                    >
                      <span class="chord-display">
                        <For each={chordKeys(trigger())}>
                          {(key) => <kbd class="chord-key">{key}</kbd>}
                        </For>
                      </span>
                    </Show>
                  </button>
                }
              >
                <input
                  ref={triggerInput}
                  class="studio-field trigger-text-input"
                  spellcheck={false}
                  autocomplete="off"
                  placeholder="e.g. gm, sig, addr, intro_pitch"
                  value={trigger()}
                  onInput={(e) => updateTextTrigger(e.currentTarget)}
                />
              </Show>
            </div>
          </div>

          {/* Variable Tokens Toolbar */}
          <div class="token-bar">
            <span class="token-bar-label">Insert Token:</span>
            <button class="token-btn" onClick={() => insertToken("{date}")} title="Current Date (YYYY-MM-DD)">
              {`{date}`}
            </button>
            <button class="token-btn" onClick={() => insertToken("{time}")} title="12-Hour Time with AM/PM (e.g. 1:15 AM)">
              {`{time}`}
            </button>
            <button class="token-btn" onClick={() => insertToken("{clipboard}")} title="Current Clipboard Text">
              {`{clipboard}`}
            </button>
            <button class="token-btn token-btn-all" onClick={() => setShowTokensModal(true)} title="View All Tokens & Modifiers">
              All Tokens
            </button>
          </div>

          {/* Giant Multiline Expansion Canvas */}
          <div class="giant-editor-container">
            <textarea
              ref={expansionTextarea}
              class="giant-editor-textarea"
              spellcheck={false}
              placeholder="Type or paste your complete expansion here... (multiline paragraphs, email templates, addresses, code snippets, etc.)"
              value={expansion()}
              onInput={(e) => setExpansion(e.currentTarget.value)}
            />
          </div>

          {/* Canvas Footer with Live Metrics and Actions */}
          <div class="canvas-footer">
            <div class="canvas-footer-stats">
              <span class="stat-item">
                <strong>{linesCount()}</strong> {linesCount() === 1 ? "line" : "lines"}
              </span>
              <span class="stat-sep">•</span>
              <span class="stat-item">
                <strong>{wordsCount()}</strong> {wordsCount() === 1 ? "word" : "words"}
              </span>
              <span class="stat-sep">•</span>
              <span class="stat-item">
                <strong>{charsCount()}</strong> chars
              </span>

              <Show when={formSuccess()}>
                <span class="success-message">
                  <IconCheck /> {formSuccess()}
                </span>
              </Show>
              <Show when={formError()}>
                <span class="error-message">
                  <IconClose /> {formError()}
                </span>
              </Show>
            </div>

            <div class="canvas-actions">
              <Show when={!isCreatingNew() && selectedTrigger()}>
                <button
                  type="button"
                  class="btn-delete-active"
                  onClick={() => selectedTrigger() && deleteSnippet(selectedTrigger()!)}
                >
                  Delete
                </button>
              </Show>
              <button
                type="button"
                class="btn-save-snippet"
                disabled={!canSave()}
                onClick={saveOrAdd}
              >
                {isCreatingNew() ? "Create Snippet" : "Save Changes"}
              </button>
            </div>
          </div>
        </section>
      </main>

      {/* Settings Modal Overlay */}
      <Show when={showSettings()}>
        <div class="settings-modal-backdrop" onClick={() => setShowSettings(false)}>
          <div class="settings-modal-card" onClick={(e) => e.stopPropagation()}>
            <div class="settings-modal-header">
              <h3>Preferences & Engine Settings</h3>
              <button class="modal-close-btn" onClick={() => setShowSettings(false)} title="Close">
                <IconClose />
              </button>
            </div>

            <div class="settings-modal-body">
              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Text Expansion Engine</span>
                  <span class="pref-desc">Global background keyboard hook listener</span>
                </div>
                <button
                  class="switch"
                  role="switch"
                  aria-checked={enabled()}
                  onClick={toggle}
                >
                  <span class="knob" />
                </button>
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Launch at Login</span>
                  <span class="pref-desc">Start HyperType minimized in tray on Windows boot</span>
                </div>
                <button
                  class="switch"
                  role="switch"
                  aria-checked={autostart()}
                  onClick={toggleAutostart}
                >
                  <span class="knob" />
                </button>
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Default Insertion Method</span>
                  <span class="pref-desc">{insertSub(insertMode() ?? "auto", autoWords())}</span>
                </div>
                <div
                  class="seg seg-mini seg-3"
                  role="group"
                  data-pos={INSERT_MODES.indexOf(insertMode() ?? "auto")}
                >
                  <span class="seg-thumb" aria-hidden="true" />
                  <For each={INSERT_MODES}>
                    {(m) => (
                      <button
                        type="button"
                        classList={{ active: insertMode() === m }}
                        onClick={() => changeInsertMode(m)}
                      >
                        {INSERT_LABEL[m]}
                      </button>
                    )}
                  </For>
                </div>
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Auto-Paste Word Threshold</span>
                  <span class="pref-desc">
                    <input
                      class="value-edit"
                      type="text"
                      inputmode="numeric"
                      value={autoWords()}
                      onFocus={(e) => e.currentTarget.select()}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                      }}
                      onBlur={(e) =>
                        commitValueText(
                          e.currentTarget,
                          1,
                          100,
                          autoWords(),
                          commitAutoPasteWords,
                        )
                      }
                    />{" "}
                    words (pastes if &gt; {autoWords()}, types if ≤ {autoWords()})
                  </span>
                </div>
                <input
                  class="slider"
                  type="range"
                  min="1"
                  max="100"
                  step="1"
                  value={autoWords()}
                  onInput={(e) => setAutoWordsDrag(Number(e.currentTarget.value))}
                  onChange={(e) => commitAutoPasteWords(Number(e.currentTarget.value))}
                />
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Typing Replay Speed</span>
                  <span class="pref-desc">
                    <input
                      class="value-edit"
                      type="text"
                      inputmode="numeric"
                      value={wpm()}
                      onFocus={(e) => e.currentTarget.select()}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                      }}
                      onBlur={(e) =>
                        commitValueText(e.currentTarget, 100, 1500, wpm(), commitWpm)
                      }
                    />{" "}
                    words per minute
                  </span>
                </div>
                <input
                  class="slider"
                  type="range"
                  min="100"
                  max="1500"
                  step="50"
                  value={wpm()}
                  onInput={(e) => setWpmDrag(Number(e.currentTarget.value))}
                  onChange={(e) => commitWpm(Number(e.currentTarget.value))}
                />
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Paste Shortcut</span>
                  <span class="pref-desc">Sent when inserting via clipboard paste</span>
                </div>
                <div
                  class="seg seg-mini seg-3 seg-combo"
                  role="group"
                  data-pos={PASTE_COMBOS.indexOf(status()?.paste_combo ?? "ctrl_v")}
                >
                  <span class="seg-thumb" aria-hidden="true" />
                  <For each={PASTE_COMBOS}>
                    {(c) => (
                      <button
                        type="button"
                        classList={{ active: status()?.paste_combo === c }}
                        onClick={() => changePasteCombo(c)}
                      >
                        {COMBO_LABEL[c]}
                      </button>
                    )}
                  </For>
                </div>
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Clipboard Restore Delay</span>
                  <span class="pref-desc">
                    <input
                      class="value-edit value-edit-wide"
                      type="text"
                      inputmode="numeric"
                      value={restoreMs()}
                      onFocus={(e) => e.currentTarget.select()}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                      }}
                      onBlur={(e) =>
                        commitValueText(
                          e.currentTarget,
                          3000,
                          15000,
                          restoreMs(),
                          commitRestoreDelay,
                        )
                      }
                    />{" "}
                    ms until previous clipboard restores
                  </span>
                </div>
                <input
                  class="slider"
                  type="range"
                  min="3000"
                  max="15000"
                  step="100"
                  value={restoreMs()}
                  onInput={(e) => setRestoreDrag(Number(e.currentTarget.value))}
                  onChange={(e) => commitRestoreDelay(Number(e.currentTarget.value))}
                />
              </div>

              <div class="pref-row">
                <div class="pref-info">
                  <span class="pref-title">Keyboard Capture & Hook Health</span>
                  <span class="pref-desc">
                    <span
                      class="hook-status-badge"
                      classList={{
                        "hook-active": !!status()?.hook_active,
                        "hook-inactive": !status()?.hook_active,
                      }}
                    >
                      <span class="hook-dot" />
                      {status()?.hook_active ? "Active" : "Detached"}
                    </span>
                    {" · "}
                    {(status()?.hook_events ?? 0).toLocaleString()} events captured
                    <Show when={hookMessage()}>
                      <span class="hook-msg"> — {hookMessage()}</span>
                    </Show>
                  </span>
                </div>
                <button
                  type="button"
                  class="btn-repair-hook"
                  onClick={handleRepairHook}
                  disabled={repairingHook()}
                >
                  {repairingHook() ? "Repairing..." : "Repair Hook"}
                </button>
              </div>
            </div>

            <div class="settings-modal-footer">
              <div class="footer-version-group">
                <span class="app-version-tag">HyperType v{status()?.version ?? "1.1.3"}</span>
                <Show when={!updateInfo()}>
                  <button
                    type="button"
                    class="btn-check-updates"
                    onClick={() => checkForUpdate(true)}
                    disabled={updateChecking()}
                  >
                    <IconRefresh classList={{ spinning: updateChecking() }} />
                    {updateStatusText() || (updateChecking() ? "Checking..." : "Check for updates")}
                  </button>
                </Show>
              </div>

              <Show when={updateInfo()}>
                <div class="update-install-action">
                  <button
                    type="button"
                    class="btn-install-update"
                    onClick={installDownloadedUpdate}
                    disabled={updateDownloading()}
                  >
                    <IconDownload />
                    {updateDownloading()
                      ? `Downloading (${updateProgress()}%)`
                      : `Update to v${updateInfo()!.version}`}
                  </button>
                </div>
              </Show>

              <button class="btn-quit-app" onClick={() => api.quit()}>
                Quit HyperType
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Interactive Token Catalog Modal */}
      <Show when={showTokensModal()}>
        <div class="settings-modal-backdrop" onClick={() => setShowTokensModal(false)}>
          <div class="settings-modal-card tokens-catalog-card" onClick={(e) => e.stopPropagation()}>
            <div class="settings-modal-header">
              <h3>Dynamic Insert Tokens & Modifiers</h3>
              <button class="modal-close-btn" onClick={() => setShowTokensModal(false)} title="Close">
                <IconClose />
              </button>
            </div>

            <div class="tokens-catalog-body">
              <p class="tokens-catalog-intro">
                Click any token to insert it directly into your expansion canvas. You can also mix offsets like <code>{`{date+3d}`}</code> or flags like <code>{`{date:us}`}</code>.
              </p>

              <div class="token-category-group">
                <span class="token-cat-title">DATE & RELATIVE MATH</span>
                <div class="token-pills-grid">
                  <button class="token-pill-card" onClick={() => { insertToken("{date}"); setShowTokensModal(false); }}>
                    <code>{`{date}`}</code>
                    <span>Today (YYYY-MM-DD)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date+1d}"); setShowTokensModal(false); }}>
                    <code>{`{date+1d}`}</code>
                    <span>Tomorrow</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date-1d}"); setShowTokensModal(false); }}>
                    <code>{`{date-1d}`}</code>
                    <span>Yesterday</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date+7d}"); setShowTokensModal(false); }}>
                    <code>{`{date+7d}`}</code>
                    <span>Next week (+7 days)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date+1m}"); setShowTokensModal(false); }}>
                    <code>{`{date+1m}`}</code>
                    <span>Next month (+30 days)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date+1y}"); setShowTokensModal(false); }}>
                    <code>{`{date+1y}`}</code>
                    <span>Next year (+365 days)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date:text}"); setShowTokensModal(false); }}>
                    <code>{`{date:text}`}</code>
                    <span>Full text date (Thursday, Aug 20...)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date:us}"); setShowTokensModal(false); }}>
                    <code>{`{date:us}`}</code>
                    <span>US format (MM/DD/YYYY)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{date:eu}"); setShowTokensModal(false); }}>
                    <code>{`{date:eu}`}</code>
                    <span>European format (DD/MM/YYYY)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{day}"); setShowTokensModal(false); }}>
                    <code>{`{day}`}</code>
                    <span>Day name (e.g. Thursday)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{month}"); setShowTokensModal(false); }}>
                    <code>{`{month}`}</code>
                    <span>Month name (e.g. August)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{year}"); setShowTokensModal(false); }}>
                    <code>{`{year}`}</code>
                    <span>4-digit year (YYYY)</span>
                  </button>
                </div>
              </div>

              <div class="token-category-group">
                <span class="token-cat-title">TIME & OFFSETS</span>
                <div class="token-pills-grid">
                  <button class="token-pill-card" onClick={() => { insertToken("{time}"); setShowTokensModal(false); }}>
                    <code>{`{time}`}</code>
                    <span>12h Time with AM/PM</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{time:24}"); setShowTokensModal(false); }}>
                    <code>{`{time:24}`}</code>
                    <span>24h Time (HH:MM)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{time+1h}"); setShowTokensModal(false); }}>
                    <code>{`{time+1h}`}</code>
                    <span>1 Hour Later</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{time+30m}"); setShowTokensModal(false); }}>
                    <code>{`{time+30m}`}</code>
                    <span>30 Minutes Later</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{datetime}"); setShowTokensModal(false); }}>
                    <code>{`{datetime}`}</code>
                    <span>Date & Time combined</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{timestamp}"); setShowTokensModal(false); }}>
                    <code>{`{timestamp}`}</code>
                    <span>Unix epoch seconds</span>
                  </button>
                </div>
              </div>

              <div class="token-category-group">
                <span class="token-cat-title">CLIPBOARD & AI PROMPT MODIFIERS</span>
                <div class="token-pills-grid">
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard}`}</code>
                    <span>Raw clipboard text</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:quote}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:quote}`}</code>
                    <span>Quote block (&gt; line)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:code}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:code}`}</code>
                    <span>Markdown code block (```)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:bullets}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:bullets}`}</code>
                    <span>Bullet list (- line)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:upper}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:upper}`}</code>
                    <span>UPPERCASE</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:lower}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:lower}`}</code>
                    <span>lowercase</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:trim}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:trim}`}</code>
                    <span>Trimmed whitespace</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:oneline}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:oneline}`}</code>
                    <span>Single line (removes PDF enters)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{clipboard:json}"); setShowTokensModal(false); }}>
                    <code>{`{clipboard:json}`}</code>
                    <span>JSON escaped string</span>
                  </button>
                </div>
              </div>

              <div class="token-category-group">
                <span class="token-cat-title">ENVIRONMENT & GENERATORS</span>
                <div class="token-pills-grid">
                  <button class="token-pill-card" onClick={() => { insertToken("{active_app}"); setShowTokensModal(false); }}>
                    <code>{`{active_app}`}</code>
                    <span>Active app executable (e.g. Code.exe)</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{window_title}"); setShowTokensModal(false); }}>
                    <code>{`{window_title}`}</code>
                    <span>Active window title</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{uuid}"); setShowTokensModal(false); }}>
                    <code>{`{uuid}`}</code>
                    <span>Random UUID v4</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{random_pin}"); setShowTokensModal(false); }}>
                    <code>{`{random_pin}`}</code>
                    <span>Random 6-digit number</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{delimiter}"); setShowTokensModal(false); }}>
                    <code>{`{delimiter}`}</code>
                    <span>AI prompt security barrier</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{username}"); setShowTokensModal(false); }}>
                    <code>{`{username}`}</code>
                    <span>Windows username</span>
                  </button>
                  <button class="token-pill-card" onClick={() => { insertToken("{computer}"); setShowTokensModal(false); }}>
                    <code>{`{computer}`}</code>
                    <span>PC Hostname</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
