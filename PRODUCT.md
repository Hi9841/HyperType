# Product

## Register

product

## Users

Windows power users: developers, support agents, ops people who type the same
things dozens of times a day. They summon the manager window from the system
tray for fifteen seconds at a time, over whatever they were doing (a dark IDE
at night, a bright browser at noon), to add or remove a snippet or pause the
engine, then close it. Nobody "lives" in this window.

## Product Purpose

HyperType is a resident text expander for Windows. A lean Rust core watches
the keyboard and replaces triggers (`gm` → `Good morning`) instantly, anywhere.
The UI exists only to manage the snippet library and the engine state. Success
is measured in the engine's numbers: 0% idle CPU, 16 MB RAM, 4 MB binary,
expansion faster than perception. The window must feel like it belongs to that
machine.

## Brand Personality

Instrument, not app. Precise, quiet, immediate. The window opens ready,
responds in the same frame, and never makes the user watch it work. Confidence
comes from restraint: one accent, hairline borders, monospace where data lives.

## Anti-references

- Electron-app heaviness: splash states, skeleton theater, spinners for local
  operations that complete in microseconds.
- The 2024 AI default kit: violet/purple accent, glassmorphism, gradient
  decoration, oversized rounded cards.
- Raycast/Linear cosplay. Same fluency, not their identity.
- Consumer-app cheer: mascots, confetti, exclamation points.

## Design Principles

1. **Speed is the brand.** Every interaction lands within one frame of intent.
   Optimistic updates over refetch-and-wait; no spinner may appear for a local
   IPC call.
2. **The engine state is the hero.** Running vs. paused is the one piece of
   global state; it owns the accent color. When the engine is off, the chrome
   cools to neutral.
3. **Density over chrome.** This is a data surface (triggers → expansions).
   Monospace triggers, tight rows, hover-revealed actions. No decoration that
   isn't state.
4. **Keyboard first.** The whole flow (add, record, delete, toggle) is
   operable without the mouse, with visible focus.
5. **Practice what you preach.** The UI plane stays as lean as the Rust plane:
   system fonts, zero UI dependencies, CSS-only motion, a bundle measured in
   kilobytes.

## Accessibility & Inclusion

WCAG 2.1 AA: body text ≥4.5:1 (including placeholders), state never conveyed
by color alone (Running/Paused is always labeled), `prefers-reduced-motion`
alternatives for all animation, full keyboard operability with
`:focus-visible` rings.
