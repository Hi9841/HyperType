const included = [
  "Full Windows app (text expansion engine + tray manager)",
  "Unlimited snippets on your machine",
  "Text triggers and keyboard shortcuts",
  "Auto / Paste / Type insert methods",
  "Export and import your snippet library",
  "Launch at login",
  "Updates for this major version",
  "One-time payment — no subscription",
] as const;

type PricingProps = {
  onBuy: () => void;
};

export function Pricing({ onBuy }: PricingProps) {
  return (
    <section
      id="pricing"
      className="border-b border-sep"
      aria-labelledby="pricing-title"
    >
      <div className="page-shell section-pad">
        <div className="mx-auto max-w-2xl text-center">
          <p className="label-soft">Pricing</p>
          <h2
            id="pricing-title"
            className="display mt-2 text-[clamp(1.75rem,3.2vw,2.35rem)] text-ink"
          >
            One price. Own it forever.
          </h2>
          <p className="lede mx-auto mt-3">
            No monthly fee for typing less. Pay once, install on your Windows
            PC, and keep every hour you get back.
          </p>
        </div>

        <div className="mx-auto mt-12 max-w-md">
          <div className="overflow-hidden rounded-[12px] border border-accent/45 bg-raise/40 shadow-[0_0_0_1px_color-mix(in_srgb,var(--color-accent)_18%,transparent),0_24px_60px_-28px_rgba(124,45,255,0.4)]">
            <div className="border-b border-sep bg-accent/10 px-6 py-6">
              <div className="flex items-end justify-between gap-4">
                <div className="text-left">
                  <div className="text-[13px] font-semibold text-accent-ink">
                    HyperType
                  </div>
                  <div className="mt-1 font-display text-[15px] text-ink-2">
                    Windows · lifetime license
                  </div>
                </div>
                <div className="text-right">
                  <div className="display text-[44px] leading-none tracking-tight text-ink tabular-nums">
                    $19
                  </div>
                  <div className="mt-1 text-[12px] text-ink-3">one-time</div>
                </div>
              </div>
            </div>

            <ul className="space-y-3 px-6 py-6">
              {included.map((item) => (
                <li
                  key={item}
                  className="flex items-start gap-2.5 text-[14px] text-ink-2"
                >
                  <Check />
                  <span>{item}</span>
                </li>
              ))}
            </ul>

            <div className="border-t border-sep px-6 py-5">
              <button
                type="button"
                onClick={onBuy}
                className="btn btn-primary btn-lg btn-block"
              >
                Buy HyperType — $19
              </button>
              <p className="mt-3 text-center text-[13px] text-ink-3">
                Checkout wires up next. Download unlocks after payment.
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function Check() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      className="mt-0.5 shrink-0 text-accent-ink"
      aria-hidden
    >
      <path
        d="M3.5 8.5 6.5 11.5 12.5 4.5"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
