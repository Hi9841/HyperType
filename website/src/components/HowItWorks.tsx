const steps = [
  {
    n: "01",
    title: "Add a snippet",
    body: "Pick a short trigger and the text it should become. Emails, addresses, signatures, support replies — anything you type more than once.",
    demo: (
      <div className="font-data text-[13px]">
        <div className="text-ink-3">Trigger</div>
        <div className="mt-1 text-accent-ink">gm</div>
        <div className="mt-3 text-ink-3">Expands to</div>
        <div className="mt-1 text-ink">Good morning,</div>
      </div>
    ),
  },
  {
    n: "02",
    title: "Type it anywhere",
    body: "In your browser, IDE, email client, or chat — type the trigger at a word boundary. HyperType replaces it before you finish thinking about it.",
    demo: (
      <div className="space-y-2 font-data text-[13px]">
        <div className="rounded-[7px] border border-line bg-field px-3 py-2 text-ink-2">
          gm<span className="caret" aria-hidden />
        </div>
        <div className="text-center text-[11px] text-ink-3">instant</div>
        <div className="rounded-[7px] border border-accent/40 bg-raise px-3 py-2 text-ink">
          Good morning,
        </div>
      </div>
    ),
  },
  {
    n: "03",
    title: "Keep the hours",
    body: "The same ten phrases, dozens of times a day. HyperType stays in the tray, uses almost no memory, and stays out of the way until you need it.",
    demo: (
      <ul className="space-y-2 text-[13px]">
        {[
          ["Idle CPU", "~0%"],
          ["Memory", "~16 MB"],
          ["Install size", "~4 MB"],
          ["Expansion", "sub-frame"],
        ].map(([label, value]) => (
          <li
            key={label}
            className="flex items-center justify-between gap-3 border-b border-sep pb-2 last:border-0 last:pb-0"
          >
            <span className="text-ink-3">{label}</span>
            <span className="font-data font-semibold text-accent-ink tabular-nums">
              {value}
            </span>
          </li>
        ))}
      </ul>
    ),
  },
] as const;

export function HowItWorks() {
  return (
    <section id="how" className="border-b border-sep" aria-labelledby="how-title">
      <div className="page-shell section-pad">
        <div className="max-w-2xl">
          <p className="label-soft">How it works</p>
          <h2
            id="how-title"
            className="display mt-2 text-[clamp(1.75rem,3.2vw,2.35rem)] text-ink"
          >
            Three steps. Zero friction.
          </h2>
          <p className="lede mt-3">
            No new app to live in. No cloud account to babysit. Create shortcuts
            once — then type them forever, in every Windows app.
          </p>
        </div>

        <ol className="step-rail mt-12 grid gap-4 md:grid-cols-3 md:gap-5">
          {steps.map((step) => (
            <li
              key={step.n}
              className="relative flex flex-col rounded-[12px] border border-line bg-raise/40 p-5"
            >
              <span className="relative z-10 grid h-8 w-8 place-items-center rounded-full border border-line-2 bg-bg font-data text-[11px] font-semibold text-accent-ink">
                <span className="sr-only">Step </span>
                {step.n}
              </span>
              <h3 className="display mt-4 text-[18px] text-ink">{step.title}</h3>
              <p className="mt-2 flex-1 text-[15px] leading-relaxed text-ink-2">
                {step.body}
              </p>
              <div className="mt-5 rounded-[8px] border border-sep bg-bg p-4">
                {step.demo}
              </div>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
