type FooterProps = {
  onBuy: () => void;
};

export function Footer({ onBuy }: FooterProps) {
  return (
    <footer className="border-t border-sep">
      <div className="page-shell flex flex-col gap-8 py-12 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex items-start gap-2.5">
          <img
            src="/logo-mark.png"
            alt=""
            width={18}
            height={18}
            className="mt-0.5 h-[18px] w-[18px]"
          />
          <div>
            <div className="font-display text-[13px] font-semibold text-ink">
              HyperType
            </div>
            <div className="mt-0.5 text-[13px] text-ink-3">
              Type less. Everywhere.
            </div>
            <p className="mt-3 max-w-xs text-[13px] leading-relaxed text-ink-2">
              Instant text expansion for Windows. One-time purchase. Snippets
              stay on your machine.
            </p>
          </div>
        </div>

        <nav
          className="flex flex-wrap gap-x-5 gap-y-1 text-[14px] text-ink-2"
          aria-label="Footer"
        >
          <a
            href="#how"
            className="inline-flex min-h-11 items-center transition-colors hover:text-ink"
          >
            How it works
          </a>
          <a
            href="#features"
            className="inline-flex min-h-11 items-center transition-colors hover:text-ink"
          >
            Features
          </a>
          <a
            href="#pricing"
            className="inline-flex min-h-11 items-center transition-colors hover:text-ink"
          >
            Pricing
          </a>
          <button
            type="button"
            onClick={onBuy}
            className="inline-flex min-h-11 items-center text-left transition-colors hover:text-ink"
          >
            Buy — $19
          </button>
        </nav>

        <p className="text-[13px] text-ink-3 sm:text-right">
          © {new Date().getFullYear()} HyperType
          <br />
          Windows
        </p>
      </div>
    </footer>
  );
}
