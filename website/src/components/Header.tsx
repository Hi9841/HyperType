const nav = [
  { href: "#how", label: "How it works" },
  { href: "#features", label: "Features" },
  { href: "#pricing", label: "Pricing" },
] as const;

export function Header() {
  return (
    <header className="sticky top-0 z-50 border-b border-sep bg-bg/90 backdrop-blur-md">
      <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-5 sm:px-8">
        <a href="#top" className="flex items-center gap-2.5">
          <img
            src="/logo-mark.png"
            alt=""
            width={22}
            height={22}
            className="h-[22px] w-[22px]"
          />
          <span className="font-display text-[15px] font-semibold tracking-tight text-ink">
            HyperType
          </span>
        </a>

        <nav className="hidden items-center gap-7 md:flex" aria-label="Primary">
          {nav.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className="text-[13px] text-ink-2 transition-colors duration-150 hover:text-ink"
            >
              {item.label}
            </a>
          ))}
        </nav>

        <a
          href="#pricing"
          className="rounded-[7px] bg-accent px-3.5 py-1.5 text-[13px] font-semibold text-ink transition-opacity duration-150 hover:opacity-90"
        >
          Buy — $19
        </a>
      </div>
    </header>
  );
}
