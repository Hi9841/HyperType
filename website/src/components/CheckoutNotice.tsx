type CheckoutNoticeProps = {
  open: boolean;
  onClose: () => void;
};

export function CheckoutNotice({ open, onClose }: CheckoutNoticeProps) {
  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/70 p-5"
      role="dialog"
      aria-modal="true"
      aria-labelledby="checkout-title"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-[12px] border border-line-2 bg-raise p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2
          id="checkout-title"
          className="font-display text-[18px] font-semibold text-ink"
        >
          Checkout coming next
        </h2>
        <p className="mt-2 text-[14px] leading-relaxed text-ink-2">
          Payment and download unlock will be wired in a follow-up. For now
          this button is a placeholder so the marketing page is ready.
        </p>
        <p className="mt-3 text-[13px] text-ink-3">
          Planned flow: pay $19 once → download installer unlocks.
        </p>
        <button
          type="button"
          onClick={onClose}
          className="mt-5 w-full rounded-[7px] bg-accent py-2.5 text-[14px] font-semibold text-ink transition-opacity hover:opacity-90"
        >
          Got it
        </button>
      </div>
    </div>
  );
}
