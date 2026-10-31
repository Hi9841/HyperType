# Progress - HyperType full GPUI port

Plan: [task_plan.md](task_plan.md). Evidence: [findings.md](findings.md).

## 2026-10-30 - Phase 0: feasibility and plan (complete)
- Read the spec (App.tsx, styles.css, ipc.ts, shortcut.ts, Tauri shell and
  config) and the existing GPUI crate. Found the earlier port follows a
  stack.menu redesign; the owner now wants a 1:1 copy, so the visual
  direction in `PLAN.md` is superseded.
- Wrote `FEASIBILITY.md` (verdict: migrate now, judge first),
  `task_plan.md`, `findings.md`, `port-progress.md` (named so because `PROGRESS.md` exists and Windows names are case-insensitive).
- T3 catalog: grok-4.5 has no `xhigh`; grok-4.6 / 4.7 do. Asked the owner.

### Build baseline (blaze)
| Command | Result |
| --- | --- |
| `blaze check -p hypertype` | FAIL - ra-jit: no `blaze-proc-macro-srv.exe` beside ra-jit (Windows releases do not ship it) |
| `RA_JIT_PROC_MACRO_SERVER=0 blaze check -p hypertype` | FAIL 21.5 s - build script output `serde_core .../out/private.rs` not found under `target/rust-analyzer-rustc` |
| `blaze build -p hypertype` | FAIL exit 127, 2m31s cold, no message; stops at `Compiling hypertype v1.1.8 (cratespp)`; no new exe (target/debug/hypertype.exe still 2026-10-22) |

Owner rule: no cargo fallback without approval. Reported to the owner.

### Gate
Phase 0 ends here. Phase 1 starts when the owner answers the open questions
in `task_plan.md`.

### Failed attempts / notes
- Writing `migration/progress.md` overwrote the tracked `migration/PROGRESS.md`
  (Windows names are case-insensitive). Restored with `git checkout`; no loss.
  Rule for agents: this task's log is `port-progress.md`; never create a
  file whose name differs from an existing one only by case.

## 2026-10-30 - Owner answers
- Implementers: grok-4.5 at `high`. Build tool: cargo, not blaze
  (blaze rows above are history only).
- `cargo check -p hypertype`: pass, 13 s.

## 2026-10-30 - Phase 1: parity judge (in progress)
- Delegated to grok / grok-4.5 / reasoningEffort high (verified in the
  delegate result). clientRequestId `hypertype-gpui-phase1-judge-r1`.
  taskId `node:delegated-task:command%3Amcp%3A7fb0d3ca-23ca-41c1-8ab5-be785a9a9b5c%3Adelegate-task%3Ahypertype-gpui-phase1-judge-r1`.
- Scope: states.md, scenarios.md, reference capture (system Edge, display
  scale), PrintWindow capture, `parity` cargo feature, diff, VALIDATION.md.
- Next: read-only review of the result, then the Phase 1 gate.
- Result 2026-10-31: task completed with no final message, but files exist:
  `migration/parity/` (states.md, scenarios.md, scripts, 11 reference PNGs,
  VALIDATION.md, README.md), `crates/app/src/parity.rs`, `parity` feature,
  `hypertype_core::load_state_only()`. Small clippy edits in settings.rs,
  tokens.rs (files to be replaced later anyway).
- Agent numbers: noise 0.000%, broken CSS 3.126%, current GPUI 6.449%,
  proposed threshold 0.50%. Display scale 1. Edge absent; Chrome 155 used.

### Phase 1 review (coordinator, read-only) - 3 defects
- R1 BLOCKER metric is blind to dark fills: diff/default-selected.png marks
  only text and edges. Sidebar #0c0c0f and row #141418 vs GPUI fills are not
  flagged (pixelmatch YIQ threshold 0.1). This palette is all dark grays,
  so wrong surfaces would pass. A full redesign scores only 6.4%.
- R2 BLOCKER stale reference: served `dist/` from 2026-10-19; `src/`
  changed 2026-10-27 (drag fixes, slide animation).
- R3 BLOCKER mid-animation captures: settings-modal.png shows the modal at
  partial opacity (fadeIn 150 ms). Hover/seg/switch transitions also at risk;
  the recorder pulse is infinite.
- Rust changes reviewed: OK (feature-gated, `start()` behavior unchanged).
- Fix round 1 delegated (grok-4.5 high, verified): clientRequestId
  `hypertype-gpui-phase1-judge-r2`, taskId suffix `...hypertype-gpui-phase1-judge-r2`.
  Scope: R1 fill metric + validation (e)(f)(g), R2 rebuild + commit in meta,
  R3 wait for animations / freeze infinite ones, table fix.

## 2026-10-31 - Phase 1 accepted, Phase 2-3 started
- Fix round 1 result: fill metric (4x4 box, tol 3/255) + detail (pixelmatch
  0.1). Thresholds detail <= 1.0%, fill <= 3.0%. Validation: noise 0/0,
  broken 3.1/11.7 FAIL, GPUI-old 6.4/96.0 FAIL, surface 0/38.6 FAIL,
  border +4 0/0 PASS (1px borders below fill resolution), grayscale text
  0.66/2.56 PASS. Reference rebuilt from 137ac57; modal capture opaque
  (checked by eye). Accepted.
- Risk: raster-noise fill 2.56% is close to the 3.0% bar. Recalibrate with
  the first faithful GPUI frame (S1 title bar); owner sign-off on images is
  the final judge.
- Commits on main: 6fcaebe (parity feature), 2dcf293 (plan + judge),
  eb8d9f0 (RULEBOOK). Generated gpui/ and diff/ outputs are gitignored.
- Owner: "You can use more than 1 grok subagent". T3 worktree isolation
  fails ("Unable to resolve the parent checkout commit": thread root is the
  outer folder, not the repo), so worktrees are made by hand in `../wt/`.
- Running in parallel (all grok-4.5 high, verified):
  | Task | clientRequestId | Worktree / branch |
  | --- | --- | --- |
  | S7a updater backend | hypertype-gpui-s7a-updater-r1b | wt/s7a, port/s7a-updater |
  | S1 foundation | hypertype-gpui-s1-foundation-r1 | wt/s1, port/s1-foundation |
  | RULEBOOK review (read-only) | hypertype-gpui-rulebook-review-r1 | main checkout, no edits |

## 2026-10-31 - RULEBOOK review r1 and amendment A1
- Review (grok-4.5 high, read-only): 3 blockers, 9 major, 3 minor. Verified
  by coordinator in gpui-pre 0.3.9 source: BoxShadow fields
  (style.rs:349-358), prompt signature (window.rs:6667), no path-prompt
  filters (platform.rs:2802), frameless hit test (windows events.rs:987),
  focus_visible exists (div.rs:1295). All 15 accepted.
- A1 committed 693bb00: frozen Studio/Editor/update API, 4.1 behavioral
  contracts, API shapes, rfd mandatory, D10-D11, instant hover (D3 mandatory).
- S1 steered with the A1 summary (message hypertype-gpui-s1-steer-a1).
- Owner rule: every implementer result gets a grok-4.7 xhigh review.

## 2026-10-31 - S7a done, under review
- S7a commit eaff319 on port/s7a-updater: updater.rs (ureq 2.12 rustls,
  minisign-verify 0.2, semver, base64). Probe: 1.1.7 -> 1.1.8 found,
  2,396,282 bytes, signature OK; 99.0.0 -> no update; tamper -> rejected.
  Clippy pass (default + update-probe). Release exe 16,190,976 bytes before
  and after (updater not yet called, removed by LTO).
- Coordinator read: verify happens before the temp write; NSIS args
  `/P /UPDATE /R /ARGS` + escaped argv, ShellExecuteW "open", then exit(0).
  Open point: feed version string used in the temp path (path injection?).
- Review r1 by grok-4.7 xhigh (verified): clientRequestId hypertype-gpui-s7a-review-r1.
- S1 asked (title bar detail ~2%, fill ~6%). Answer: fix pill height and
  any logo/dot/button geometry, treat PNG/SVG anti-aliasing as D4, re-diff
  once, then finish checks and commit; classify each remaining difference
  (geometry/color vs raster) with coordinates. Note: region % is not
  comparable to the full-frame calibration; recalibrate per region later.

## 2026-10-31 - S1 done, under review
- S1 commit 8b567fc on port/s1-foundation. Clippy pass (default + parity),
  resize hit-test OK on 4 edges + 4 corners (WM_NCHITTEST probes), Studio
  frozen API + Editor/update stubs, superseded modules deleted.
- Parity default-selected: full 4.18/59.3 (stubs), title bar 2.04/5.95,
  sidebar 4.99/58.3, editor 3.97/65.1 (detail/fill %).
- Coordinator zoom of the title bar (5x): layout, pill, buttons, colors
  match. Not raster, contrary to S1's report: O1 status dot lacks the 1px
  SURFACE_1 ring and its glow is weaker ((15,27,21) vs (17,46,27));
  O2 logo paler/softer (likely poor downscale). O3 "HyperType" ~2px wider =
  D2 letter-spacing (expected).
- Review r1 by grok-4.7 xhigh (verified): hypertype-gpui-s1-review-r1.
- S1 amendment requests: (1) region bars - decision pending: per-region bars
  after the first complete frame; until then coordinator zoom + owner
  sign-off rule D4 calls. (2) note that DefWindowProc gives side/bottom
  resize hits on this host - accepted as fact, add to RULEBOOK at next amendment.
- Plan: S2+S3, S4, S5+S6 start in parallel after S1's fix round merges
  (they build on S1 widgets; a widget API change would break them).

## 2026-10-31 - S7a review r1 ruled, fix round running
- grok-4.7 xhigh review: VERDICT CHANGES, 20 findings (5 major, 11 minor,
  4 nit), no blocker. Signature path, NSIS args, platform order, semver
  compare confirmed correct against Tauri source.
- Rulings: accept 1-18 and 20; reject 19 (user agent, no compat effect).
  14/15 also bind S7b: relaunch only after the instance guard and hook are
  released; all updater calls on a worker thread.
- Coordinator open point (version in temp path) closed: semver forbids
  '/' and '..'.
- Fix round: grok-4.5 high, hypertype-gpui-s7a-fix-r1, wt/s7a, new commit.
- S7a fix commit 6d3c00b (agent ended without a report). Coordinator
  verified: clippy default + update-probe pass; live probes 1.1.7 and
  v1.1.7 -> version "1.1.8", unique temp dir `hypertype-1.1.8-updater-*`,
  signature OK, installer not run; 99.0.0 -> no update; tamper -> rejected.
  Code has 204, dynamic shape + name alias, 512 MiB cap, HKCU proxy + env,
  native-certs, MZ gate, chrono RFC3339, URL-free errors, redirects 10,
  temp cleanup on launch failure. Not yet executed: 204 / dynamic / size /
  pub_date / 404 local cases.
- Verification review r2 (grok-4.7 xhigh): hypertype-gpui-s7a-review-r2.

## 2026-10-31 - S1 review r1 ruled, fix round running
- grok-4.7 xhigh review of 8b567fc: VERDICT CHANGES, 9 major, 8 minor, 2 nit.
  Confirmed O2 (logo 128px drawn at 18px, no mipmaps) and O3 (D2). Corrected
  O1: the dark ring exists (my sample was 1px shifted); the real defect is
  the glow: GPUI blur_radius is a Gaussian sigma (CSS 6px -> sigma 3.96).
  Also: drag area too small, icon hover color blocked, segmented/switch
  motion ids shared or snapping, slider loses drags outside its box,
  value_edit metrics, modal fade never runs, Shell observer overwrites Status.
- Rulings: accept 1-18; 19 no change.
- A2 committed a53ff5f: RULEBOOK section 7 rules 11-16 (blur->sigma, motion
  ids, enter animations, non-hover transitions required, drag capture,
  pre-scaled PNGs, centering remainder).
- Fix round: grok-4.5 high, hypertype-gpui-s1-fix-r1, wt/s1, new commit.

## 2026-10-31 - S7a verification review r2, fix round 2
- grok-4.7 xhigh r2 (ended without a final message; result read from its
  thread): 18/20 fixed, 2 partial; executed 204, dynamic feed, lying
  Content-Length, 512 MiB cap, pub_date, missing target, 404 text, 15 s
  deadline against a local server in %TEMP%\hypertype-updater-review.
  New: redirect off-by-one (ureq stops at 9), ProxyOverride loses to env
  proxy, raw version in error text, body timeout shown as I/O error,
  NO_PROXY ignored, IPv6 host parse. All accepted.
- Fix round 2: grok-4.5 high, hypertype-gpui-s7a-fix-r2, wt/s7a.
- Process note: delegated agents sometimes end without a final message;
  always read the child thread (t3_thread_read, view messages) for results.

## 2026-10-31 - S7a merged
- Fix round 2 commit 498a041: redirects(11) = 10 followed; SystemProxy
  {Proxy, Direct, Unset} with env proxy only for Unset; NO_PROXY; IPv6
  host; URL-free version error; body timeout -> "timed out". Proven with
  the reviewer's local server repros (redir10 ok, redir11 fail, NO_PROXY
  direct, version URL hidden, slow body timed out at 15.1 s) + live probes.
- Coordinator reviewed the r2 diff (117 lines) instead of a third Grok
  round: every item had an executed repro from the r2 reviewer's server.
- Rebased onto main and fast-forwarded: 184dd5f, 97751cc, 737ca73. Clippy
  pass on main. Pushed.
- S7b (UI wiring) must: run updater calls on a worker thread; before
  relaunch/installer exit, release the instance guard and stop the hook.

## 2026-10-31 - S1 fix round 1 done, verification running
- fa2af52 on port/s1-foundation: all 18 items addressed. Title bar parity
  detail 2.036 -> 1.895 %, fill 5.947 -> 5.000 %. Drag HTCAPTION on the bar,
  HTCLIENT on buttons (WM_NCHITTEST). Close hover and value_edit Enter/blur
  checked from code only. Agent says dot core still ~1px high.
- Coordinator 5x zoom: logo now bright and sharp (match); dot ring looks like
  a cross and the dot is ~1px high (to rule).
- Verification review r2: grok-4.7 xhigh, hypertype-gpui-s1-review-r2.
- Wrote `migration/briefs.md`: S2+S3, S4, S5+S6, S7b scopes for the fan-out.

## 2026-10-31 - S1 verification r2 ruled; fan-out started early
- grok-4.7 xhigh r2 of fa2af52: CHANGES. Live checks pass: drag map
  (HTCAPTION/HTCLIENT), close hover red + white X, dot position aligned,
  logo BOX assets exact. Blocking: F1 slider click/capture, F2 Kit Input
  padding on value_edit, F3 dot ring rasterizes as a plus, F4 icon slot on
  two buttons. Nits N4-N7 (file-wide allows, Kit clamps spring overshoot,
  fadeIn keyword ease-out, logo 1.25x/1.5x). All accepted.
- A3 committed be00714: rules 17-20 + region acceptance rule.
- Running (all grok-4.5 high, verified):
  | Task | clientRequestId | Worktree / branch (base) |
  | --- | --- | --- |
  | S1 fix r2 | hypertype-gpui-s1-fix-r2 | wt/s1 port/s1-foundation |
  | S4 editor | hypertype-gpui-s4-editor-r1 | wt/s4 port/s4-editor (fa2af52) |
  | S6 tokens modal | hypertype-gpui-s6-tokens-r1 | wt/s6 port/s6-tokens (fa2af52) |
- Why early: S4 and S6 do not use the widgets S1 r2 changes (slider,
  value_edit, icon buttons). S2 (uses btn_create_first) and S5 (slider,
  value_edit, btn_check_updates) wait for S1 r2.
- Merge order: S1 (rebased onto main, conflicts in main.rs/Cargo.toml with
  the updater expected) -> S4, S6 rebased onto main.

## 2026-10-31 - S4 editor done, under review
- f773437 on port/s4-editor (no final message; report read from thread).
  editor.rs ~976 lines + 1 line in studio.rs (Editor::new takes window;
  S1-owned file, needed for Input/Textarea states - judge at merge).
  Clippy pass. Region 340,42,620,598 detail/fill: default 2.65/7.20,
  new 4.17/11.41, save-disabled 4.17/11.41, shortcut-empty 3.62/9.68,
  recording 3.95/11.00. Agent claimed all blocks D2/D4. Create/Edit/Rename
  not driven end to end.
- Coordinator side-by-side (new-snippet) rejects the claim: C1 All Tokens
  button not right-aligned/wrong font+color, C2 textarea placeholder
  indented, lower, not wrapping, C3 1-2px vertical drift (line height).
- Process lesson: implementers over-claim the region rule. Coordinator
  must look at a side-by-side before any review/merge.
- Review r1: grok-4.7 xhigh, hypertype-gpui-s4-review-r1.

## 2026-10-31 - S6 tokens modal done, under review
- 2b642ec on port/s6-tokens (tokens_modal.rs only). Clippy pass. Card
  region 140,38,680,564: detail 6.97 %, fill 22.18 %. Insert/close checked
  via parity-only states. It duplicated modal_frame (hardcoded 500px/0.85)
  and reserves a 15px gutter with pr(35).
- Coordinator side-by-side: grid, columns, pills, colors, title match.
  Defects: C1 intro inline code lacks its SURFACE_2 pill background,
  C2 no scrollbar although the body overflows, C3 row pitch 60.0 vs 60.3
  (2px drift by the second group).
- Request (S1-owned widgets.rs): modal_frame(width, max_height) params.
  Plan: apply after S1 fix r2 lands, then S6 fix round removes the copy.
- Review r1: grok-4.7 xhigh, hypertype-gpui-s6-review-r1.

## 2026-10-31 - S1 fix round 2 done, verification r3 running
- 77af696 on port/s1-foundation: slider capture + click commit on release
  (Rc<Cell> drag state keyed per id), value_edit padding 0 / h 16.8,
  dot = two filled circles, icon slots on two buttons (spin frozen in
  parity), item-level allows, unclamped spring(t), keyword ease-out fade,
  logo 18/23/27/36. Parity-only widget-lab state for slider/value_edit/buttons.
  Title bar 1.895 / 4.962 %; 131 failing blocks: D2 104, D4 27, defect 0.
  Kit Input ignores synthetic SendInput (F2 commit proven by code path).
- Coordinator check: brand 5x zoom matches, dot round; widget-lab f1a shows
  thumb at 25 % and committed=25; value_edit dashed underline, no padding.
- Answered the agent's question: click with no movement must commit on
  release (spec input+change).
- Verification r3: grok-4.7 xhigh, hypertype-gpui-s1-review-r3.
- Merge note: S1 r2 and S4 both touch studio.rs (S4: Editor::new(window));
  resolve at S4 rebase.

## 2026-10-31 - S6 review r1 ruled; A4; S6 fix round
- grok-4.7 xhigh review of 2b642ec: CHANGES. Confirmed C1 (inline code
  chips), C2 (no scrollbar), C3 (row pitch 60 vs 60/61, TIME title 2px
  high). Also: pill label hover color (needs element id), Escape not
  handled, fade curve (already fixed by S1 r2), copied modal_frame drifted,
  insert check cannot fail while editor insert is a stub.
- Ruling on Escape: App.tsx has NO Escape handler on either modal (only the
  recorder, line 408). The RULEBOOK rule "Escape closes" was mine and wrong
  for a 1:1 copy. Corrected in A4: Escape does nothing; keyboard parity =
  native button tab stops, Enter/Space, focus-visible ring.
- A4 committed eb479b0 + 61693eb: rules 21-25 (keyboard focus parity,
  inline boxes are elements, fractional line heights, group hover needs an
  id, scrollbar visible at rest).
- S6 rebased onto S1 r2 (ae91379). Fix round: grok-4.5 high,
  hypertype-gpui-s6-fix-r1, with an approved ADDITIVE exception in
  widgets.rs: modal_frame_sized (modal_frame unchanged) and removal of any
  Escape-close in the shared frame.
- Follow-up for S1 (titlebar buttons) and S4 (editor controls): rule 21
  keyboard focus parity must be checked in their next rounds.

## 2026-10-31 - S4 review r1 ruled; A5; S4 fix round
- grok-4.7 xhigh review of f773437: CHANGES, verified live with real input.
  C1-C3 confirmed (All Tokens style, textarea Kit padding +10/+7 and no
  placeholder wrap, line-box drift 1-2px). Plus: click-away overlay fails on
  resize and right click and swallows the click; bare keys type into the
  textarea during recording; focus moves do not cancel; recorder lacks the
  focus ring; char count in bytes (café😀 = 9, spec 6); long error pushes
  Save off-canvas; load() blurs the search field. Passed live: Esc cancel,
  left click-away, mode switch, 64 code-point cap, Create + "Snippet
  created!" 3 s, token at caret, duplicate error, Delete prompt.
- Rulings: accept 1-10; 11 (busy never painted) no change: native save is
  synchronous. studio.rs one-line Editor::new(window) accepted.
- A5 committed a61a609: displayed lengths are UTF-16 units (S2 row badge too).
- S4 rebased onto S1 r2 (f86bfdd, clean). Fix round: grok-4.5 high,
  hypertype-gpui-s4-fix-r1. Also applies rule 21 keyboard parity.
- parity/.gitignore: zooms/ and package-lock.json ignored.
- S4 asked to copy shared widgets into editor.rs for focus support: refused.
  Rule 21 focus for shared widgets (segmented, btn_*, switch, slider,
  value_edit) will be added ONCE in widgets.rs (S1 owner) as a follow-up;
  S4 adds focus only to editor-owned controls and lists the need.

## 2026-10-31 - S4 fix round 1 done; verification + judge boxes running
- 6904321 (editor.rs only): All Tokens builder, textarea inset + own
  wrapping placeholder, explicit line boxes, capture-phase click-away
  listener (overlay removed), key swallow, recorder focus/blur cancel,
  UTF-16 chars, footer flex/truncate, load() no blur, editor-owned tab
  stops. Region detail/fill now: default 2.39/6.26, new 3.46/9.41,
  save-disabled 3.46/9.41, shortcut-empty 2.91/7.69, recording 3.24/8.96.
- Coordinator side-by-side: very close; remaining drift label/trigger/token
  row 1px high, textarea and footer ~2px high (CSS line boxes: e.g. inline
  12px span in a 13px/1.5 block uses a 19.5px strut).
- Request from S4: focus support in widgets::segmented / btn_* (rule 21) -
  coordinator to schedule ONE widgets.rs pass for all shared controls.
- Running: S4 verification r2 (grok-4.7 xhigh, hypertype-gpui-s4-review-r2);
  judge tool "reference element boxes" (grok-4.5 high,
  hypertype-gpui-judge-boxes-r1, wt/judge, port/judge-boxes) so slices
  match exact reference geometry instead of guessing line boxes.

## 2026-10-31 - Judge boxes merged (A6)
- 9e4dcf0: capture-reference writes reference/<state>.boxes.json (every
  visible element rect + computed style, text line rects); `bun run boxes`
  CLI. All 11 reference PNGs bit-identical (SHA-256). Coordinator verified:
  0 PNG files in the diff; CLI prints h2 at y=62 h=24 (16px/24px).
  Reviewed by the coordinator (judge tooling, no app code).
- Note: settings modal has 9 pref-rows (brief said 8; agent was right).
- A6: boxes are the geometry target in RULEBOOK section 10 and briefs.md.

## 2026-10-31 - S6 fix round 1 done; verification r2 running
- 92f412f: intro chips as elements, Kit scrollbar Always, row starts
  191/251/312/372 via alternating pill heights 52/53, label hover via a
  hovered_token state (group names with braces failed), no Escape, tab
  stops, additive modal_frame_sized (modal_frame unchanged). Card region
  5.96 / 19.93 %. Enter/Space wired but not delivered by SendInput.
- Coordinator zoom: K1 intro line break differs (GPUI breaks before "or
  flags"; agent hand-placed the break because line 1 overflowed), K2 the
  committed capture shows a hovered pill (pointer left over the window),
  K3 some rows ~1px high.
- Judge TODO: the run script must park the pointer outside the window
  before each capture (K2).
- Verification r2: grok-4.7 xhigh, hypertype-gpui-s6-review-r2.

## 2026-10-31 - S1 verification r3 ruled; S1 fix round 3
- grok-4.7 xhigh r3 of 77af696 (result read from thread): CHANGES.
  Verified live: click jump + single commit on release, value_edit padding
  0 and Enter clamp (Kit input CAN be driven by SendInput once the window
  is active), round dot, button icons, spring peak 1.0204, keyword ease-out,
  logo pick 18/23/27/36, widget-lab cfg-gated.
  Blocking: slider move outside the track frozen (is_hovered on a stale
  captured HitboxId); window controls 1px high (5 blocks clear on a 1px
  shift) so "defect 0" was false. Also STUDIO text 1px low (coordinator:
  baseline move = geometry). One idle parity process exited -1 (unexplained).
- Fix round 3 (grok-4.5 high, hypertype-gpui-s1-fix-r3): slider drag,
  control row + pill text against `bun run boxes`, idle stability, and
  rule-21 keyboard focus for ALL shared widgets (segmented, switch, slider
  keys like input range, buttons) - the queued shared-focus pass.

## 2026-10-31 - S4 verification r2 ruled; S4 fix round 2
- grok-4.7 xhigh r2 of 6904321 (from thread): CHANGES. Live-verified 8/10:
  All Tokens, textarea inset + wrapping placeholder, capture listener
  (right click, token click inserts after cancel, resize), bare V, Esc,
  chord, recorder ring, blur + WM_ACTIVATE cancel, UTF-16 count (a😀b = 4),
  footer truncation with Save on canvas, no blur in load(); no listener leak.
- Blocking: textarea top 2px early (sub-hint pinned 18 vs CSS strut 19.5 ->
  GPUI rounds to 20; token buttons 24 vs 25); Tab not consumed while
  recording (spec consumes every key).
- Correction: my "footer 2px high" note was wrong; footer y matches.
- Fix round 2: grok-4.5 high, hypertype-gpui-s4-fix-r2.

## 2026-10-31 - S6 verification r2 ruled; A7; S6 fix round 2
- grok-4.7 xhigh r2 of 92f412f (parked pointer): CHANGES. Pass: scrollbar
  Always + wheel + fixed header, label hover INK_2, no Escape, Tab/Enter/
  Space after focus, fade curve, modal_frame unchanged, no Escape in frame.
  Blocking: hand-placed intro break (" or flags" fits line 1 with 5.19px
  slack), chips 14 vs 16px, pill stack 1px high (3 rows miss 1px), first
  Tab with nothing focused does nothing (Kit binds Tab only in "Root"
  context). Nits: hovered_token duplicates hover (brace claim false);
  fixed 52/53 heights would clip wrapped labels.
- Rulings: 1-3, 5 to S6; 4 moved to S1 (window-wide); 6 keep + comment.
- A7 committed 5e08dad: rule 21a - all tab_index(0) (GPUI sorts by index
  path then insertion order), first Tab from no focus goes to the first
  stop in DOM order, modals do not trap focus; inline text wraps per word.
- S1 fix round 3 steered with A7 (shared widgets tab_index 0 + root focus
  for the first Tab). S6 fix round 2: grok-4.5 high, hypertype-gpui-s6-fix-r2.

## 2026-10-31 - S4 accepted (pending merge after S1)
- d43f751: sub-hint line box 20 (-1 margin), token + All Tokens h 25, Tab
  consumed while recording. Offsets: all 12 elements within 1px (11 exact).
- Coordinator verification (instead of a third Grok round; small diff with
  measured proof): fresh captures from the d43f751 build reproduce
  default-selected 1.760/3.892 and shortcut-recording 2.284/5.467; stacked
  side-by-side shows all rows aligned; token buttons a few px wider = mono
  glyph advance (D4, as ruled for All Tokens in review r2). Remaining
  blocks: D2 (title/label letter-spacing) + D4. ACCEPTED.
- Merge order: S1 (r3) -> rebase S4 (shared widget signatures may change)
  -> S6 -> then S2 and S5 start.

## 2026-10-31 - S6 fix round 2; text width investigation
- 0664a94: per-word wrapping intro, chips h16, intro min_h 39 (pill tops
  within 1px of boxes), group_hover with ids (hovered_token removed),
  pills tab_index(0), 52/53 comment. Agent claimed the intro wrap matches.
- Coordinator fresh capture: region 6.015 / 14.230 %. Zoom REJECTS the wrap
  claim: GPUI line 2 = "flags like {date:us}.", reference "like {date:us}.".
  With natural wrapping, GPUI text is >5px wider over the line. Same family
  of symptom: All Tokens +2px, mono token buttons wider.
- Hypothesis: systematic advance-width gap from a GPUI shaping setting
  (kern/liga features, Segoe UI Variable opsz axis, advance rounding).
  Investigation task: grok-4.5 high, hypertype-gpui-textwidth-r1, in wt/s6,
  temporary changes reverted, fix delivered as a diff (likely S1 tokens.rs).
- S6 holds its merge until the text width outcome.

## 2026-10-31 - S1 fix round 3 done; S1 verification r4; S2 started
- 25a9b42 (mod.rs, studio.rs, titlebar.rs, widget_lab.rs, widgets.rs):
  slider live drag outside the track, control row top(1)/h28, pill text,
  shared widget focus (tab_index 0, Enter/Space, slider keys, rings), Studio
  root focus + "Studio" key context so the first Tab reaches the title bar
  (verified live), no modal trap, idle 180 s OK. Title bar 1.513 / 4.621 %.
  OPEN: switch Enter/Space after click-focus does not toggle.
- Verification r4: grok-4.7 xhigh, hypertype-gpui-s1-review-r4 (incl. the
  switch diagnosis, Studio root focus regressions).
- S2+S3 sidebar started from 25a9b42 (widget signatures unchanged in r3):
  grok-4.5 high, hypertype-gpui-s2-sidebar-r1, wt/s2, port/s2-sidebar.
- S5 (settings) waits for the text-width outcome (keeps <= 3 Grok agents).

## 2026-10-31 - Text width root cause found
- Investigation (grok-4.5 high, hypertype-gpui-textwidth-r1; worktree left
  clean): GPUI DirectWrite applies `kern` only when listed in FontFeatures
  (gpui-pre-windows direct_write.rs:1797-1833); FontFeatures::default() is
  empty (font_features.rs:8-14). Chromium kerns by default. With kern=1 UI
  widths match to <0.01px (All Tokens 50.499 vs 50.500, title 231.06 vs
  231.06, intro run 502.667 vs 502.672); mono unchanged and already exact.
  Remaining title gaps = CSS letter-spacing exactly (D2). No opsz axis API
  (<0.3% effect). Text element widths are ceil()'d (text.rs:772): per-word
  atoms waste up to 1px each.
- Fix diff: tokens::font_features_ui() = [("kern",1)], set on the Studio root
  + mono_zero_advance. Goes into S1's next round (S1 owns the files).
- S6 intro: rebuild with few segments ("Click ... like ", chip, " or flags ",
  "like ", chip, ".") after the kern fix lands.
- S5 started: worktree wt/s5, port/s5-settings from 25a9b42.

## 2026-10-31 - S5 settings done, under review
- ba89727 (settings_modal.rs, 1159 lines): all 9 rows, footer, 1.5 s poll,
  background repair/elevate, optimistic setters, tab_index 0, Kit scrollbar
  Always. Card 48/544 matches; card region 6.379/16.667 %. Live: engine,
  insert mode, paste combo, sliders -> temp settings.json. Value-edit typing
  not driven (harness); login + run-as-admin by code only (by design).
  Requests: modal_frame no-Escape + overflow_hidden (S6 has them),
  SettingsModal::new(window) (inputs today created lazily in render).
- Coordinator side-by-side: structure matches; rows drift up 2/4/6px going
  down (value_edit rows: ref pref-row h 58.3 vs 56.5).
- Process: S5 and S2 wrote captures into the MAIN checkout's gitignored
  parity/gpui/ (shared folder). briefs.md now says: outputs stay in the
  worktree or %TEMP%; only the boxes CLI runs from main.
- Review r1: grok-4.7 xhigh, hypertype-gpui-s5-review-r1.

## 2026-10-31 - S1 verification r4 ruled; S1 fix round 4
- grok-4.7 xhigh r4 of 25a9b42: CHANGES (1 blocker). PASS live: slider hold
  outside (live 100 / 1 during hold), slider keys, control row + pill text
  y within 1px, idle 3 min, first Tab -> settings then tree order, Shift+Tab,
  no modal trap, value_edit typing (Ctrl+A, 42, Enter -> committed=42).
  Blocker: switch Enter/Space toggles twice (GPUI click fires on KeyUp AND our
  on_key_down toggles). Segmented has the same double path (harmless).
  Rulings: accept; value_edit has no ring by spec (`outline: none`) - nit
  rejected; modal_frame Escape left to S6 merge.
- Incident: one reviewer launch (via WMI) lacked APPDATA -> 2 parity lines
  in the REAL %APPDATA%\HyperType\hypertype.log (8016-8017) and
  "single-instance triggered" lines (8001, 8019) = the installed app was asked
  to show its window. snippets.json (Oct 27) and settings.json (Sep 9)
  untouched. Briefs now require APPDATA in every launch + log line check.
- Fix round 4 (grok-4.5 high, hypertype-gpui-s1-fix-r4): switch/segmented
  single activation path, kern font features (proven diff), SettingsModal::new
  takes window.

## 2026-10-31 - S2+S3 sidebar done, under review
- 8538236 (sidebar.rs ~1394 lines + rfd 0.16): search, New/Export/Import
  (rfd on a background thread), toast, list, rows, empty state, pointer drag
  reorder with capture, missed-release commit, blur cancel, grip arrows,
  lock while searching, tab_index 0. Region 0,42,340,598 detail/fill:
  default 3.97/9.73, row-hover 4.13/10.41, filter 2.72/5.69, no-match
  1.48/3.92, empty 1.71/4.20. Scenarios pass: search, drag 1->3, keyboard,
  lock, delete Cancel, export, import; delete OK partial (GPUI prompt).
- Agent claimed "all D4"; coordinator side-by-side REJECTS: C1 search icon
  missing, C2 scrollbar painted without overflow, C3 header ~2px high and
  rows pitch 63 vs 63.5 (drift), C4 delete X opacity hack to check.
- Review r1: grok-4.7 xhigh, hypertype-gpui-s2-review-r1 (APPDATA + real
  log line counts required).
- S1 fix round 4 steered: Sidebar::new also takes window.

## 2026-10-31 - BLOCKED: Grok balance exhausted (HTTP 402)
- "API error (status 402 Payment Required): Grok Build usage balance
  exhausted". Failed: S2 review r1 (hypertype-gpui-s2-review-r1), S1 fix
  round 4 (hypertype-gpui-s1-fix-r4). S5 review r1 still shows running; expect
  the same failure.
- S1 round 4 left UNCOMMITTED, correct partial edits in wt/s1 (on 25a9b42):
  switch + segmented on_key_down removed (single activation via on_click),
  tokens::font_features_ui() kern=1 on Studio root + mono_zero_advance,
  SettingsModal::new(studio, window, cx). Missing: Sidebar::new(window),
  build/clippy, live switch Enter/Space check, title bar re-measure.
- State of slices: S1 r4 partial (above); S4 accepted (d43f751, waits for S1
  merge); S6 needs intro segments after kern (0664a94); S2 8538236 needs
  review + fixes (C1 icon, C2 scrollbar, C3 drift); S5 ba89727 under review
  (row drift); S7a merged; S7b not started; Phase 6 scope question open.
- Waiting for the owner: top up Grok, or switch implementers/reviewers.

## 2026-10-31 - Coordinator-direct work: S1, S4, S6 merged
- Owner: Grok exhausted -> coordinator does the work directly; Phase 6 in
  scope (confirm release-affecting steps first).
- S1 round 4 finished by coordinator (d77fd09 -> rebased 521c7a5): switch +
  segmented single activation (live: click->true, Enter->false, Space->true,
  Space->false; real log 8025 -> 8025), kern on Studio root + mono metric
  font, Sidebar::new + SettingsModal::new take window. Title bar 1.491/4.583;
  HyperType ink 65 vs 64 and STUDIO 36 vs 38 = exactly the CSS letter-spacing
  (D2). Clippy default/parity/update-probe pass. Rebased onto main (one
  main.rs conflict: keep `mod ui; mod updater;`). MERGED.
- S4 rebased with --onto (studio.rs ctor line: all three take window).
  Editor region after kern: default 1.357/3.247, new 2.081/5.492,
  save-disabled same, shortcut-empty 1.815/4.817, recording 1.815/4.908.
  MERGED 5eec9de.
- S6 rebased (widgets.rs doc conflict); coordinator fix c0b6395: intro in
  six segments (no per-word ceil waste) -> wrap matches the reference;
  removed duplicate tab_index and the extra Enter/Space handlers on the
  modal close button and pills (single on_click path). Card region
  5.624/13.876 (remaining D2 titles, D4 glyphs, D5 scrollbar). MERGED.
- Next: S2 (sidebar: C1 icon, C2 scrollbar, C3 drift, Sidebar::new window),
  S5 (settings: row drift, ctor), S7b, full parity + scenarios, Phase 6.

## 2026-10-31 - S2 merged (coordinator)
- Rebased 8538236 -> 9ab976d (Cargo.toml: keep minisign/semver/serde + rfd;
  Cargo.lock regenerated from main + rfd).
- Coordinator fix 41f6a62: search icon painted after the input frame (was
  hidden: GPUI paints in tree order); scrollbar only on overflow; list
  header CSS padding (title y 151) and 31px; rows alternate 59/60 so tops
  = 178/241/305/368/432 exactly like Chromium's half-up rounding (GPUI snaps
  toward zero: a 59.5 row drifted 0.5px per row); drag pitch 60
  (offsetHeight); search InputState created in new().
- Region 0,42,340,598 detail/fill: default 2.299/5.835 (was 3.97/9.73),
  row-hover 2.295/5.898, search-filter 1.282/3.569, no-match 1.418/3.773,
  empty 1.675/4.086. Remaining: D4 glyphs, D2 list title, caret in the
  search-filter capture (blink phase artifact).
- Live (coordinator drivers in %TEMP%\claude, APPDATA temp copy, real log
  8025 -> 8025): drag row1 by 2 rows -> addr,sig,gm,brb,omw; by 1 row ->
  addr,gm,sig,brb,omw; delete row 2 + Enter on the native "Warning" prompt ->
  addr removed; + Escape -> unchanged.
- Lesson for later slices: fractional CSS boxes need Chromium half-up
  rounding emulation on GPUI (it snaps toward zero).

## 2026-10-31 - S5 merged; shrink + gutter fixes
- S5 rebased (f9f12fb) + coordinator fix 429a3b8: rows flex_shrink_0 (Taffy
  squeezed them to fit the scroll body: that was the 2/4/6px drift) with
  heights on Chromium's rounded grid -> separators 171/241/330/402/474 exact;
  value-edit inputs created in new(); render writes a value-edit only when
  its status value changes (before, Enter's blur re-sync undid the typed
  value, so nothing saved). Live: words 999+Enter -> 100 saved; WPM
  999+Enter -> 999 saved (temp settings.json; real log 8025 -> 8025).
  Card 6.753/14.171 (from 8.495/21.476). MERGED.
- 3fd...: sidebar rows + token groups flex_shrink_0 (A9 rule 23b); sidebar
  list reserves a 15px scrollbar gutter while overflowing. 15-snippet seed:
  row tops stay 178/241/305/368/432/495/559/622; scrollbar in the gutter.
  Regression parity unchanged (sidebar 2.299/5.835 ..., tokens 5.624/13.876).
- ALL UI slices merged: S1 S2 S4 S5 S6 + S7a. Next: S7b update UI glue,
  then full parity + scenario sweep, release build metrics, Phase 6.

## 2026-10-31 - S7b update UI merged (coordinator)
- b4832d1: ui/update.rs per App.tsx 53-120 (silent check at open, skipped in
  parity builds; manual texts; 4 s clear with a generation guard; install with
  live percent; "Update failed: <45>"; background executor). updater.rs:
  UpdateInfo Clone; stale "future UI" allows removed (signature + relaunch
  keep reasoned allows). Clippy default/parity/update-probe pass.
- Live with a fake local feed (version 9.9.9, bogus signature; server in
  %TEMP%\claude\feed_server.py, stopped after): manual check -> footer
  "Update to v9.9.9"; install -> GET fake.exe, signature rejected before any
  write (no hypertype-9.9.9-updater-* dir), process alive, button back to
  idle. Parity state update-available shows the "Update v1.1.9" pill.
  Real log 8025 -> 8025.

## 2026-10-31 - Full parity sweep on main (Phase 5)
- Judge bug fixed (run.ts): fresh APPDATA seed per state; before, the
  empty-library state emptied the library for every later state.
- Full-frame sweep at 1c9ed79 (detail / fill %, bars 1 / 3):
  | state | size | detail % | detail px | fill % | fill blocks | pass | note |
  | --- | --- | ---: | ---: | ---: | ---: | --- | --- |
  | default-selected | 960x640 | 1.678% | 10307 | 4.115% | 1580/38400 | FAIL |  |
  | row-hover | 960x640 | 1.676% | 10299 | 4.130% | 1586/38400 | FAIL |  |
  | new-snippet | 960x640 | 2.113% | 12984 | 6.039% | 2319/38400 | FAIL |  |
  | shortcut-mode-empty | 960x640 | 1.955% | 12013 | 4.891% | 1878/38400 | FAIL |  |
  | shortcut-recording | 960x640 | 1.955% | 12013 | 4.924% | 1891/38400 | FAIL |  |
  | search-filter | 960x640 | 1.332% | 8186 | 3.417% | 1312/38400 | FAIL |  |
  | search-no-match | 960x640 | 1.392% | 8555 | 3.612% | 1387/38400 | FAIL |  |
  | empty-library | 960x640 | 1.908% | 11721 | 4.987% | 1915/38400 | FAIL |  |
  | settings-modal | 960x640 | 3.220% | 19782 | 11.195% | 4299/38400 | FAIL |  |
  | tokens-modal | 960x640 | 3.667% | 22527 | 11.607% | 4457/38400 | FAIL |  |
  | save-disabled | 960x640 | 2.113% | 12984 | 6.039% | 2319/38400 | FAIL |  |
- Remaining classes: D2 letter-spacing (titles, labels, brand), D4 glyph
  raster, D5 scrollbar look, D1 no backdrop blur (most of the modal states'
  fill: the page behind the backdrop is blurred in Chromium). Geometry:
  measured within 1px per slice (title bar, editor, sidebar, settings rows,
  token rows).
- Incident note: at 11:03:30 the INSTALLED HyperType (old PID 2376) started an
  elevated child (PID 19256) - its own Run-as-Admin path (UAC consent needed;
  synthetic input cannot click UAC). Its window was focused at 11:03:21 while
  coordinator click drivers ran; both apps center the same 960x640 window.
  Cannot rule out a stray synthetic click, but Run as Admin sits below the
  fold of its settings list. snippets.json / settings.json untouched.
