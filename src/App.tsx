import { createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import {
  api,
  onEnabledChanged,
  win,
  type InsertMode,
  type PasteCombo,
  type SnippetView,
  type TriggerKind,
} from "./lib/ipc";
import { chordFromEvent, chordKeys } from "./lib/shortcut";
import logo from "./assets/logo.png";

const INSERT_MODES: InsertMode[] = ["auto", "paste", "type"];
const INSERT_LABEL: Record<InsertMode, string> = { auto: "Auto", paste: "Paste", type: "Type" };
const INSERT_SUB: Record<InsertMode, string> = {
  auto: "Types short single sentences, pastes the rest",
  paste: "Expansions are pasted instantly",
  type: "Expansions are typed out key by key",
};
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
  // Live value while dragging the speed slider; null when idle.
  const [wpmDrag, setWpmDrag] = createSignal<number | null>(null);
  const [mode, setMode] = createSignal<TriggerKind>("text");
  const [trigger, setTrigger] = createSignal("");
  const [expansion, setExpansion] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [recording, setRecording] = createSignal(false);
  const [error, setError] = createSignal("");
  const [editingTrigger, setEditingTrigger] = createSignal("");
  const [editMode, setEditMode] = createSignal<TriggerKind>("text");
  const [editTrigger, setEditTrigger] = createSignal("");
  const [editExpansion, setEditExpansion] = createSignal("");
  const [editBusy, setEditBusy] = createSignal(false);
  const [editRecording, setEditRecording] = createSignal(false);
  const [editError, setEditError] = createSignal("");
  const [transferBusy, setTransferBusy] = createSignal(false);
  const [transferMessage, setTransferMessage] = createSignal("");
  // Trigger of the row added most recently, so only that row plays its
  // entrance animation (never the initial load).
  const [fresh, setFresh] = createSignal("");
  // Trigger of the row being deleted, so it plays its exit animation before
  // it leaves the list.
  const [leaving, setLeaving] = createSignal("");
  // Drag-to-reorder state: the index being dragged, its live pixel offset
  // from where the drag started, and the slot it currently hovers over.
  const [dragIdx, setDragIdx] = createSignal<number | null>(null);
  const [dragY, setDragY] = createSignal(0);
  const [dropIdx, setDropIdx] = createSignal(0);
  let rowHeight = 36;

  let triggerInput: HTMLInputElement | undefined;
  let expansionInput: HTMLInputElement | undefined;
  let stopRecording: (() => void) | undefined;
  let stopEditRecording: (() => void) | undefined;

  onMount(() => {
    triggerInput?.focus();
    api.getAutostart().then(setAutostart).catch(() => {});
    // Keep the window in sync when the engine is toggled from the tray menu.
    const unlisten = onEnabledChanged((enabled) => {
      const s = status();
      if (s) mutateStatus({ ...s, enabled });
    });
    onCleanup(() => unlisten.then((f) => f()));
  });

  const enabled = () => status()?.enabled ?? false;
  const canAdd = () => !busy() && trigger().trim().length > 0 && expansion().length > 0;
  const canSaveEdit = () =>
    !editBusy() && editTrigger().trim().length > 0 && editExpansion().length > 0;

  onCleanup(() => {
    stopRecording?.();
    stopEditRecording?.();
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

  // Live value while dragging the restore-delay slider; null when idle.
  const [restoreDrag, setRestoreDrag] = createSignal<number | null>(null);
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

  // Inline-editable numeric values: focusing selects the whole number (the
  // "Ctrl+A and retype" feel), Enter/blur commits clamped, Esc reverts.
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

  function valueEditKeys(e: KeyboardEvent & { currentTarget: HTMLInputElement }, revert: number) {
    if (e.key === "Enter") {
      e.currentTarget.blur();
    } else if (e.key === "Escape") {
      e.currentTarget.value = String(revert);
      e.currentTarget.blur();
    }
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
    setError("");
    if (next === "text") queueMicrotask(() => triggerInput?.focus());
  }

  function startRecording() {
    if (recording()) return;
    stopEditRecording?.();
    setRecording(true);
    setTrigger("");
    setError("");

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
        expansionInput?.focus();
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

  function beginEdit(s: SnippetView) {
    stopRecording?.();
    stopEditRecording?.();
    setEditingTrigger(s.trigger);
    setEditMode(s.kind);
    setEditTrigger(s.trigger);
    setEditExpansion(s.expansion);
    setEditError("");
  }

  function cancelEdit() {
    stopEditRecording?.();
    setEditingTrigger("");
    setEditTrigger("");
    setEditExpansion("");
    setEditError("");
  }

  function switchEditMode(next: TriggerKind) {
    if (editMode() === next) return;
    stopEditRecording?.();
    setEditMode(next);
    setEditTrigger("");
    setEditError("");
  }

  function startEditRecording() {
    if (editRecording()) return;
    stopRecording?.();
    setEditRecording(true);
    setEditTrigger("");
    setEditError("");

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.code === "Escape") {
        stop();
        return;
      }
      const chord = chordFromEvent(e);
      if (chord) {
        setEditTrigger(chord);
        stop();
      }
    };
    const stop = () => {
      window.removeEventListener("keydown", onKeyDown, true);
      setEditRecording(false);
      stopEditRecording = undefined;
    };
    stopEditRecording = stop;
    window.addEventListener("keydown", onKeyDown, true);
  }

  async function add(e: Event) {
    e.preventDefault();
    const t = trigger().trim();
    const x = expansion();
    if (!t || !x || busy()) return;
    setBusy(true);
    setError("");
    try {
      await api.addSnippet(t, x, mode());
      setTrigger("");
      setExpansion("");
      setFresh(t);
      setTimeout(() => setFresh(""), 400);
      await Promise.all([refetchSnippets(), refetchStatus()]);
      if (mode() === "text") triggerInput?.focus();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function saveEdit(e: Event, oldTrigger: string) {
    e.preventDefault();
    const t = editTrigger().trim();
    const x = editExpansion();
    if (!t || !x || editBusy()) return;
    setEditBusy(true);
    setEditError("");
    try {
      await api.editSnippet(oldTrigger, t, x, editMode());
      setFresh(t);
      setTimeout(() => setFresh(""), 400);
      cancelEdit();
      await Promise.all([refetchSnippets(), refetchStatus()]);
    } catch (err) {
      setEditError(String(err));
    } finally {
      setEditBusy(false);
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
    if (editingTrigger()) return;
    if (e.button !== 0 || dragIdx() !== null) return;
    e.preventDefault();
    rowHeight = e.currentTarget.closest("li")?.offsetHeight ?? 36;
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

  // Keyboard reorder on the grip: plain ArrowUp/ArrowDown moves the row.
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

  /** Row transform while a drag is live: the dragged row follows the
      pointer, rows between the start and hover slots step aside. */
  function rowShift(index: number): string | undefined {
    const from = dragIdx();
    if (from === null) return undefined;
    if (index === from) return `transform: translateY(${dragY()}px)`;
    const to = dropIdx();
    if (from < to && index > from && index <= to) return `transform: translateY(-${rowHeight}px)`;
    if (from > to && index >= to && index < from) return `transform: translateY(${rowHeight}px)`;
    return undefined;
  }

  async function remove(t: string) {
    if (leaving() === t) return;
    if (editingTrigger() === t) cancelEdit();
    // Let the row play its short exit animation, then drop it; the refetch
    // reconciles (and restores it if the backend refused).
    setLeaving(t);
    await new Promise((r) => setTimeout(r, 160));
    setLeaving("");
    mutateSnippets((list) => list?.filter((s) => s.trigger !== t));
    const s = status();
    if (s) mutateStatus({ ...s, count: Math.max(0, s.count - 1) });
    try {
      await api.removeSnippet(t);
    } finally {
      refetchSnippets();
      refetchStatus();
    }
  }

  async function exportSnippets() {
    if (transferBusy()) return;
    setTransferBusy(true);
    setTransferMessage("");
    try {
      const path = await api.exportSnippets();
      if (path) setTransferMessage("Exported");
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
      }
    } catch (err) {
      setTransferMessage(String(err));
    } finally {
      setTransferBusy(false);
    }
  }

  return (
    <div class="app">
      <header class="titlebar" data-tauri-drag-region>
        <div class="brand" data-tauri-drag-region>
          {/* Brand mark (HyperType_Logo_Assets, gradient-clean 128px shown at
              20px). Doubles as the engine light: full color while running,
              dimmed to gray while paused. */}
          <img
            class="mark"
            classList={{ paused: !enabled() }}
            src={logo}
            alt=""
            width="20"
            height="20"
          />
          <span class="name" data-tauri-drag-region>
            HyperType
          </span>
        </div>
        <div class="win-controls">
          <button class="winbtn" aria-label="Minimize" onClick={() => win.minimize()}>
            &#xE921;
          </button>
          <button class="winbtn close" aria-label="Close" onClick={() => win.close()}>
            &#xE8BB;
          </button>
        </div>
      </header>

      <main class="content">
      <section class="group">
        <div class="card">
          <div class="setting-row">
            <div class="setting-text">
              <span class="setting-title">Text Expansion</span>
              <span class="setting-sub" classList={{ on: enabled() }}>
                {enabled()
                  ? `${status()?.count ?? 0} snippet${(status()?.count ?? 0) === 1 ? "" : "s"} active`
                  : "Paused"}
              </span>
            </div>
            <button
              class="switch"
              role="switch"
              aria-checked={enabled()}
              aria-label="Text expansion engine"
              onClick={toggle}
            >
              <span class="knob" />
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-text">
              <span class="setting-title">Launch at Login</span>
              <span class="setting-sub">Start in the background when you sign in</span>
            </div>
            <button
              class="switch"
              role="switch"
              aria-checked={autostart()}
              aria-label="Launch at login"
              onClick={toggleAutostart}
            >
              <span class="knob" />
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-text">
              <span class="setting-title">Insert Method</span>
              <span class="setting-sub">{INSERT_SUB[insertMode() ?? "auto"]}</span>
            </div>
            <div
              class="seg seg-mini seg-3"
              role="group"
              aria-label="Insert method"
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
          <Show when={insertMode() && insertMode() !== "paste"}>
            <div class="setting-row reveal">
              <div class="setting-text">
                <span class="setting-title">Typing Speed</span>
                <span class="setting-sub">
                  <input
                    class="value-edit"
                    type="text"
                    inputmode="numeric"
                    value={wpm()}
                    aria-label="Typing speed in words per minute"
                    onFocus={(e) => e.currentTarget.select()}
                    onBlur={(e) => commitValueText(e.currentTarget, 100, 1500, wpm(), commitWpm)}
                    onKeyDown={(e) => valueEditKeys(e, wpm())}
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
                aria-label="Typing speed in words per minute"
                onInput={(e) => setWpmDrag(Number(e.currentTarget.value))}
                onChange={(e) => commitWpm(Number(e.currentTarget.value))}
              />
            </div>
          </Show>
          <Show when={insertMode() && insertMode() !== "type"}>
            <div class="setting-row reveal">
              <div class="setting-text">
                <span class="setting-title">Paste Shortcut</span>
                <span class="setting-sub">Terminals often paste with Shift+Ins</span>
              </div>
              <div
                class="seg seg-mini seg-3 seg-combo"
                role="group"
                aria-label="Paste shortcut"
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
            <div class="setting-row reveal">
              <div class="setting-text">
                <span class="setting-title">Clipboard Restore</span>
                <span class="setting-sub">
                  <input
                    class="value-edit value-edit-wide"
                    type="text"
                    inputmode="numeric"
                    value={restoreMs()}
                    aria-label="Clipboard restore delay in milliseconds"
                    onFocus={(e) => e.currentTarget.select()}
                    onBlur={(e) =>
                      commitValueText(e.currentTarget, 3000, 15000, restoreMs(), commitRestoreDelay)
                    }
                    onKeyDown={(e) => valueEditKeys(e, restoreMs())}
                  />{" "}
                  ms until your old clipboard returns
                </span>
              </div>
              <input
                class="slider"
                type="range"
                min="3000"
                max="15000"
                step="100"
                value={restoreMs()}
                aria-label="Clipboard restore delay in milliseconds"
                onInput={(e) => setRestoreDrag(Number(e.currentTarget.value))}
                onChange={(e) => commitRestoreDelay(Number(e.currentTarget.value))}
              />
            </div>
          </Show>
        </div>
      </section>

      <section class="group">
        <div class="card composer-card">
          <form class="composer" onSubmit={add}>
            <div class="seg" role="group" aria-label="Trigger type" data-mode={mode()}>
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
            <Show
              when={mode() === "text"}
              fallback={
                <button
                  type="button"
                  class="field trigger-field recorder"
                  classList={{ listening: recording() }}
                  onClick={startRecording}
                >
                  <Show
                    when={!recording() && trigger()}
                    fallback={
                      <span class="recorder-hint">
                        {recording() ? "Press keys… Esc cancels" : "Record shortcut"}
                      </span>
                    }
                  >
                    <span class="chord">
                      <For each={chordKeys(trigger())}>
                        {(key) => <kbd class="keycap">{key}</kbd>}
                      </For>
                    </span>
                  </Show>
                </button>
              }
            >
              <input
                ref={triggerInput}
                class="field trigger-field"
                spellcheck={false}
                autocomplete="off"
                placeholder="Trigger, e.g. gm"
                value={trigger()}
                onInput={(e) => setTrigger(e.currentTarget.value)}
              />
            </Show>
            <span class="composer-arrow" aria-hidden="true">
              &#8594;
            </span>
            <input
              ref={expansionInput}
              class="field expansion-field"
              spellcheck={false}
              autocomplete="off"
              placeholder="Expands to, e.g. Good morning"
              value={expansion()}
              onInput={(e) => setExpansion(e.currentTarget.value)}
            />
            <button class="add" type="submit" disabled={!canAdd()}>
              Add
            </button>
          </form>
          <Show when={error()}>
            <p class="form-error" role="alert">
              {error()}
            </p>
          </Show>
        </div>
      </section>

      <section class="group library">
        <div class="group-head">
          <h2>Snippets</h2>
          <span class="count">{status()?.count ?? 0}</span>
          <div class="library-actions">
            <button type="button" onClick={exportSnippets} disabled={transferBusy()}>
              Export
            </button>
            <button type="button" onClick={importSnippets} disabled={transferBusy()}>
              Import
            </button>
          </div>
        </div>
        <Show when={transferMessage()}>
          <p class="transfer-message">{transferMessage()}</p>
        </Show>
        <div class="card">
          <ul class="list" classList={{ reordering: dragIdx() !== null }}>
            <Show
              when={(snippets()?.length ?? 0) > 0}
              fallback={
                <li class="empty">
                  <p class="empty-title">No snippets yet</p>
                  <div class="empty-demo" aria-hidden="true">
                    <kbd class="keycap">gm</kbd>
                    <span class="arrow">&#8594;</span>
                    <span>Good morning</span>
                  </div>
                  <p class="empty-line">
                    Add one above. Typing its trigger in any app replaces it
                    instantly.
                  </p>
                </li>
              }
            >
              <For each={snippets()}>
                {(s: SnippetView, i) => (
                  <Show
                    when={editingTrigger() === s.trigger}
                    fallback={
                      <li
                        class="row"
                        classList={{
                          fresh: s.trigger === fresh(),
                          leaving: s.trigger === leaving(),
                          dragging: dragIdx() === i(),
                        }}
                        style={rowShift(i())}
                      >
                        <button
                          class="grip"
                          aria-label={`Reorder ${s.trigger}. Arrow keys move it up or down`}
                          onPointerDown={(e) => startDrag(e, i())}
                          onKeyDown={(e) => gripKeys(e, i())}
                        >
                          <svg width="8" height="13" viewBox="0 0 8 13" aria-hidden="true">
                            <circle cx="2" cy="2.5" r="1.3" />
                            <circle cx="6" cy="2.5" r="1.3" />
                            <circle cx="2" cy="6.5" r="1.3" />
                            <circle cx="6" cy="6.5" r="1.3" />
                            <circle cx="2" cy="10.5" r="1.3" />
                            <circle cx="6" cy="10.5" r="1.3" />
                          </svg>
                        </button>
                        <Show
                          when={s.kind === "shortcut"}
                          fallback={<kbd class="keycap">{s.trigger}</kbd>}
                        >
                          <span class="chord">
                            <For each={chordKeys(s.trigger)}>
                              {(key) => <kbd class="keycap">{key}</kbd>}
                            </For>
                          </span>
                        </Show>
                        <span class="arrow" aria-hidden="true">
                          &#8594;
                        </span>
                        <span class="expansion" title={s.expansion}>
                          {s.expansion}
                        </span>
                        <button
                          class="iconbtn edit"
                          aria-label={`Edit ${s.trigger}`}
                          onClick={() => beginEdit(s)}
                        >
                          &#9998;
                        </button>
                        <button
                          class="iconbtn del"
                          aria-label={`Delete ${s.trigger}`}
                          onClick={() => remove(s.trigger)}
                        >
                          &#10005;
                        </button>
                      </li>
                    }
                  >
                    <li class="row row-edit" classList={{ fresh: s.trigger === fresh() }}>
                      <form
                        class="row-edit-form"
                        onSubmit={(e) => saveEdit(e, s.trigger)}
                        onKeyDown={(e) => {
                          if (e.key === "Escape" && !editRecording()) cancelEdit();
                        }}
                      >
                        <div class="row-edit-main">
                          <div class="row-edit-top">
                            <div
                              class="seg seg-mini row-kind"
                              role="group"
                              aria-label="Trigger type"
                              data-mode={editMode()}
                            >
                              <span class="seg-thumb" aria-hidden="true" />
                              <button
                                type="button"
                                classList={{ active: editMode() === "text" }}
                                onClick={() => switchEditMode("text")}
                              >
                                Text
                              </button>
                              <button
                                type="button"
                                classList={{ active: editMode() === "shortcut" }}
                                onClick={() => switchEditMode("shortcut")}
                              >
                                Shortcut
                              </button>
                            </div>
                            <Show
                              when={editMode() === "text"}
                              fallback={
                                <button
                                  type="button"
                                  class="field trigger-field row-trigger recorder"
                                  classList={{ listening: editRecording() }}
                                  onClick={startEditRecording}
                                >
                                  <Show
                                    when={!editRecording() && editTrigger()}
                                    fallback={
                                      <span class="recorder-hint">
                                        {editRecording() ? "Press keys..." : "Record shortcut"}
                                      </span>
                                    }
                                  >
                                    <span class="chord">
                                      <For each={chordKeys(editTrigger())}>
                                        {(key) => <kbd class="keycap">{key}</kbd>}
                                      </For>
                                    </span>
                                  </Show>
                                </button>
                              }
                            >
                              <input
                                class="field trigger-field row-trigger"
                                spellcheck={false}
                                autocomplete="off"
                                value={editTrigger()}
                                onInput={(e) => setEditTrigger(e.currentTarget.value)}
                              />
                            </Show>
                          </div>
                          <input
                            class="field row-expansion"
                            spellcheck={false}
                            autocomplete="off"
                            value={editExpansion()}
                            onInput={(e) => setEditExpansion(e.currentTarget.value)}
                          />
                          <Show when={editError()}>
                            <p class="form-error row-error" role="alert">
                              {editError()}
                            </p>
                          </Show>
                        </div>
                        <div class="row-actions">
                          <button
                            class="iconbtn save"
                            type="submit"
                            disabled={!canSaveEdit()}
                            aria-label={`Save ${s.trigger}`}
                          >
                            &#10003;
                          </button>
                          <button
                            class="iconbtn"
                            type="button"
                            aria-label="Cancel edit"
                            onClick={cancelEdit}
                          >
                            &#10005;
                          </button>
                        </div>
                      </form>
                    </li>
                  </Show>
                )}
              </For>
            </Show>
          </ul>
        </div>
      </section>

      <footer class="foot">
        <span class="hint">Triggers expand anywhere in Windows, at word boundaries.</span>
        <span class="ver">v{status()?.version ?? "1.0.5"}</span>
        <button class="quit" onClick={() => api.quit()}>
          Quit HyperType
        </button>
      </footer>
      </main>
    </div>
  );
}
