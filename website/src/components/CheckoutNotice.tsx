import { useEffect, useId, useRef } from "react";

type CheckoutNoticeProps = {
  open: boolean;
  onClose: () => void;
};

export function CheckoutNotice({ open, onClose }: CheckoutNoticeProps) {
  const titleId = useId();
  const descId = useId();
  const closeRef = useRef<HTMLButtonElement>(null);
  const previouslyFocused = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;

    previouslyFocused.current = document.activeElement as HTMLElement | null;
    const t = window.setTimeout(() => closeRef.current?.focus(), 0);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);

    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    return () => {
      window.clearTimeout(t);
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
      onClick={onClose}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div
        className="dialog-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <h2 id={titleId} className="display text-[18px] text-ink">
            Checkout coming next
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="btn btn-ghost -mr-1 -mt-1 rounded-[6px] px-2 py-1 text-[18px] leading-none text-ink-3"
            aria-label="Close dialog"
          >
            ×
          </button>
        </div>
        <p id={descId} className="mt-2 text-[14px] leading-relaxed text-ink-2">
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
