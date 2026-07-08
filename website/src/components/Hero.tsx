import { AppMock } from "./AppMock";

export function Hero() {
  return (
    <section
      id="top"
      className="relative overflow-hidden border-b border-sep"
    >
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute top-[-20%] left-1/2 h-[520px] w-[720px] -translate-x-1/2 rounded-full bg-accent/10 blur-[100px]" />
      </div>

      <div className="relative mx-auto grid max-w-6xl items-center gap-12 px-5 py-16 sm:px-8 sm:py-20 lg:grid-cols-[1.1fr_0.9fr] lg:gap-16 lg:py-24">
        <div>
          <p className="mb-4 text-[13px] font-semibold text-accent-ink">
            Instant text expansion for Windows
          </p>
          <h1 className="font-display text-[clamp(2.4rem,5vw,3.75rem)] leading-[1.05] font-semibold tracking-tight text-balance text-ink">
            Type less.
            <br />
            <span className="text-accent-ink">Everywhere.</span>
          </h1>
          <p className="mt-5 max-w-xl text-[17px] leading-relaxed text-ink-2 text-pretty">
            Stop retyping the same emails, addresses, and replies every day.
            HyperType turns short triggers into full text — instantly — in
            every app on your PC.
          </p>

          <div className="mt-8 flex flex-wrap items-center gap-3">
            <a
              href="#pricing"
              className="inline-flex items-center justify-center rounded-[7px] bg-accent px-5 py-2.5 text-[14px] font-semibold text-ink transition-opacity duration-150 hover:opacity-90"
            >
              Get HyperType — $19
            </a>
            <a
              href="#how"
              className="inline-flex items-center justify-center rounded-[7px] border border-line-2 bg-raise px-5 py-2.5 text-[14px] font-semibold text-ink transition-colors duration-150 hover:border-ink-3"
            >
              See how it works
            </a>
          </div>

          <ul className="mt-8 flex flex-wrap gap-x-6 gap-y-2 text-[13px] text-ink-3">
            <li className="flex items-center gap-2">
              <Dot /> One-time purchase
            </li>
            <li className="flex items-center gap-2">
              <Dot /> Works in every app
            </li>
            <li className="flex items-center gap-2">
              <Dot /> Lives in your system tray
            </li>
          </ul>
        </div>

        <div className="flex justify-center lg:justify-end">
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
