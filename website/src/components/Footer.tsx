export function Footer() {
  return (
    <footer className="border-t border-sep">
      <div className="mx-auto flex max-w-6xl flex-col gap-4 px-5 py-10 sm:flex-row sm:items-center sm:justify-between sm:px-8">
        <div className="flex items-center gap-2.5">
          <img
            src="/logo-mark.png"
            alt=""
            width={18}
            height={18}
            className="h-[18px] w-[18px]"
          />
          <div>
            <div className="font-display text-[13px] font-semibold text-ink">
              HyperType
            </div>
            <div className="text-[12px] text-ink-3">
              Type less. Everywhere.
            </div>
          </div>
        </div>

        <nav
          className="flex flex-wrap gap-x-5 gap-y-2 text-[13px] text-ink-3"
          aria-label="Footer"
        >
          <a href="#how" className="hover:text-ink">
            How it works
          </a>
          <a href="#features" className="hover:text-ink">
            Features
          </a>
          <a href="#pricing" className="hover:text-ink">
            Pricing
          </a>
        </nav>

        <p className="text-[12px] text-ink-3">
          © {new Date().getFullYear()} HyperType · Windows
        </p>
      </div>
    </footer>
  );
}
