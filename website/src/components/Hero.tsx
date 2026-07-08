import { AppMock } from "./AppMock";

type HeroProps = {
  onBuy: () => void;
};

export function Hero({ onBuy }: HeroProps) {
  return (
    <section id="top" className="relative overflow-hidden border-b border-sep">
      <div className="pointer-events-none absolute inset-0" aria-hidden>
        <div className="hero-glow absolute inset-0" />
        <div className="grid-dots absolute inset-0 opacity-80" />
      </div>

      <div className="page-shell relative grid items-center gap-12 section-pad lg:grid-cols-[1.15fr_0.85fr] lg:gap-16">
        <div>
          <p className="label-soft anim-rise">
            Instant text expansion for Windows
          </p>

          <h1 className="display anim-rise anim-delay-1 mt-4 text-[clamp(2.5rem,5.5vw,3.9rem)] text-ink">
            Type less.
            <br />
            <span className="text-accent-ink">Everywhere.</span>
          </h1>

          <p className="lede anim-rise anim-delay-2 mt-5">
            Stop retyping the same emails, addresses, and replies. HyperType
            turns short triggers into full text in every app on your PC —
            instantly, from the system tray.
          </p>

          <div className="anim-rise anim-delay-3 mt-8 flex flex-wrap items-center gap-3">
            <button type="button" onClick={onBuy} className="btn btn-primary btn-lg">
              Get HyperType — $19
            </button>
            <a href="#how" className="btn btn-secondary btn-lg">
              See how it works
            </a>
          </div>

          <ul className="anim-rise anim-delay-3 mt-8 flex flex-wrap gap-x-6 gap-y-2 text-[13px] text-ink-3">
            <li className="flex items-center gap-2">
              <Dot /> One-time purchase
            </li>
            <li className="flex items-center gap-2">
              <Dot /> Works in every app
            </li>
            <li className="flex items-center gap-2">
              <Dot /> ~16 MB RAM
            </li>
          </ul>
        </div>

        <div className="anim-rise anim-delay-2 flex justify-center lg:justify-end">
          <AppMock />
        </div>
      </div>
    </section>
  );
}

function Dot() {
  return (
    <span
      className="inline-block h-1.5 w-1.5 rounded-full bg-accent"
      aria-hidden
    />
  );
}
