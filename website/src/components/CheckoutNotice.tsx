import { useEffect, useId, useRef } from "react";

type CheckoutNoticeProps = {
  open: boolean;
  onClose: () => void;
};

const FOCUSABLE =
  'a[href], button:not([disabled]), textarea, input, select, [tabindex]:not([tabindex="-1"])';

export function CheckoutNotice({ open, onClose }: CheckoutNoticeProps) {
  const titleId = useId();
  const descId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const previouslyFocused = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;

    previouslyFocused.current = document.activeElement as HTMLElement | null;
    const focusTimer = window.setTimeout(() => closeRef.current?.focus(), 0);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }

      if (e.key !== "Tab" || !panelRef.current) return;

      const nodes = Array.from(
        panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE),
      ).filter((el) => !el.hasAttribute("disabled") && el.tabIndex !== -1);

      if (nodes.length === 0) {
        e.preventDefault();
        return;
      }

      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      const active = document.activeElement as HTMLElement | null;

      if (e.shiftKey) {
        if (active === first || !panelRef.current.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last) {
        e.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKey);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = prevOverflow;
      previouslyFocused.current?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="dialog-backdrop"
      role="presentation"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        className="dialog-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
      >
        <div className="flex items-start justify-between gap-4">
          <h2 id={titleId} className="display text-[18px] text-ink">
            Checkout coming next
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="btn btn-ghost -mr-1 -mt-1 rounded-[6px] text-[20px] leading-none text-ink-3"
            aria-label="Close dialog"
          >
            ×
          </button>
        </div>
        <p id={descId} className="mt-2 text-[15px] leading-relaxed text-ink-2">
          Payment and download unlock land in a follow-up. This button is a
          placeholder so the marketing page is ready to ship.
        </p>
        <p className="mt-3 text-[13px] text-ink-3">
          Planned flow: pay $19 once → installer download unlocks.
        </p>
        <button
          ref={closeRef}
          type="button"
          onClick={onClose}
          className="btn btn-primary btn-block mt-5"
        >
          Got it
        </button>
      </div>
    </div>
  );
}
