import { useEffect, useId, useState } from "react";

const nav = [
  { href: "#how", id: "how", label: "How it works" },
  { href: "#features", id: "features", label: "Features" },
  { href: "#pricing", id: "pricing", label: "Pricing" },
] as const;

type HeaderProps = {
  onBuy: () => void;
};

export function Header({ onBuy }: HeaderProps) {
  const [scrolled, setScrolled] = useState(false);
  const [active, setActive] = useState("top");
  const [menuOpen, setMenuOpen] = useState(false);
  const menuId = useId();

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    const ids = ["top", "how", "features", "pricing"] as const;
    const sections = ids
      .map((id) => document.getElementById(id))
      .filter((el): el is HTMLElement => el !== null);

    if (sections.length === 0) return;

    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
        if (visible[0]?.target.id) {
          setActive(visible[0].target.id);
        }
      },
      { rootMargin: "-28% 0px -55% 0px", threshold: [0, 0.2, 0.45, 1] },
    );

    for (const section of sections) observer.observe(section);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!menuOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenuOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [menuOpen]);

  const closeMenu = () => setMenuOpen(false);

  return (
    <header className={`site-header ${scrolled ? "is-scrolled" : ""}`}>
      <div className="page-shell flex h-14 items-center justify-between gap-3">
        <a
          href="#top"
          className="focus-ring flex min-h-11 items-center gap-2.5 rounded-[6px]"
          onClick={closeMenu}
        >
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

        <nav className="hidden items-center gap-6 md:flex" aria-label="Primary">
          {nav.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className="nav-link focus-ring rounded-sm px-1"
              aria-current={active === item.id ? "true" : undefined}
            >
              {item.label}
            </a>
          ))}
        </nav>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onBuy}
            className="btn btn-primary btn-sm"
          >
            Buy — $19
          </button>

          <button
            type="button"
            className="btn btn-ghost md:hidden"
            aria-expanded={menuOpen}
            aria-controls={menuId}
            aria-label={menuOpen ? "Close menu" : "Open menu"}
            onClick={() => setMenuOpen((v) => !v)}
          >
            <MenuIcon open={menuOpen} />
          </button>
        </div>
      </div>

      {menuOpen ? (
        <div id={menuId} className="page-shell mobile-nav md:hidden">
          <nav aria-label="Mobile">
            {nav.map((item) => (
              <a
                key={item.href}
                href={item.href}
                onClick={closeMenu}
                aria-current={active === item.id ? "true" : undefined}
              >
                {item.label}
              </a>
            ))}
            <button
              type="button"
              onClick={() => {
                closeMenu();
                onBuy();
              }}
            >
              Buy HyperType — $19
            </button>
          </nav>
        </div>
      ) : null}
    </header>
  );
}

function MenuIcon({ open }: { open: boolean }) {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden>
      {open ? (
        <path
          d="M5 5l10 10M15 5 5 15"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        />
      ) : (
        <>
          <path
            d="M4 6h12M4 10h12M4 14h12"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
          />
        </>
      )}
    </svg>
  );
}
