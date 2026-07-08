import { useEffect, useState } from "react";

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
  const [active, setActive] = useState<string>("top");

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
      { rootMargin: "-30% 0px -55% 0px", threshold: [0, 0.25, 0.5, 1] },
    );

    for (const section of sections) observer.observe(section);
    return () => observer.disconnect();
  }, []);

  return (
    <header
      className={`sticky top-0 z-50 border-b transition-[background-color,border-color,backdrop-filter] duration-200 ${
        scrolled
          ? "border-sep bg-bg/85 backdrop-blur-md"
          : "border-transparent bg-bg/40 backdrop-blur-sm"
      }`}
    >
      <div className="page-shell flex h-14 items-center justify-between">
        <a href="#top" className="flex items-center gap-2.5 focus-ring rounded-[6px]">
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
              className="nav-link focus-ring rounded-sm py-1"
              aria-current={active === item.id ? "true" : undefined}
            >
              {item.label}
            </a>
          ))}
        </nav>

        <button
          type="button"
          onClick={onBuy}
          className="btn btn-primary !px-3.5 !py-1.5 text-[13px]"
        >
          Buy — $19
        </button>
      </div>
    </header>
  );
}
