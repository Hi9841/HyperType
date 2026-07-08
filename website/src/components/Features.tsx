const features = [
  {
    title: "Works in every app",
    body: "Browser, Outlook, Slack, Word, your terminal — HyperType expands wherever you type on Windows. No per-app setup.",
  },
  {
    title: "Text & keyboard shortcuts",
    body: "Simple triggers like gm, or full key chords. Build a library that matches how you actually work.",
  },
  {
    title: "Insert method you control",
    body: "Auto, Paste, or Type. Tune paste shortcuts and clipboard restore so expansion behaves the way each app expects.",
  },
  {
    title: "Tray-first, always ready",
    body: "It lives in the system tray. Open the manager for fifteen seconds, add a snippet, close it. Back to your work.",
  },
  {
    title: "Ridiculously light",
    body: "Built for speed: ~0% idle CPU, ~16 MB RAM, a few megabytes on disk. No Electron bloat. No waiting spinners.",
  },
  {
    title: "Your snippets stay yours",
    body: "Snippets live on your machine. Export and import when you need to. Buy once — no subscription for your own phrases.",
  },
] as const;

const outcomes = [
  {
    t: "Emails that write themselves",
    d: "Signatures, intros, and canned replies — one trigger away.",
  },
  {
    t: "Fewer typos under pressure",
    d: "Expand perfect phrases instead of racing the keyboard.",
  },
  {
    t: "Same speed in every window",
    d: "Your shortcuts follow you across the whole desktop.",
  },
] as const;

export function Features() {
  return (
    <section
      id="features"
      className="border-b border-sep"
      aria-labelledby="features-title"
    >
      <div className="page-shell section-pad">
        <div className="max-w-2xl">
          <p className="label-soft">Features</p>
          <h2
            id="features-title"
            className="display mt-2 text-[clamp(1.75rem,3.2vw,2.35rem)] text-ink"
          >
            Built for people who type the same things all day
          </h2>
          <p className="lede mt-3">
            Support replies. Client updates. Your full address. A polite
            “thanks for waiting.” HyperType makes the boring parts of typing
            disappear.
          </p>
        </div>

        <div className="mt-12 border-b border-sep">
          {features.map((f) => (
            <article key={f.title} className="feature-row group">
              <h3 className="display text-[16px] text-ink transition-colors duration-150 group-hover:text-accent-ink">
                {f.title}
              </h3>
              <p className="text-[15px] leading-relaxed text-ink-2">{f.body}</p>
            </article>
          ))}
        </div>

        <div className="mt-12 grid gap-8 border-t border-sep pt-10 sm:grid-cols-3 sm:gap-6">
          {outcomes.map((item) => (
            <div key={item.t}>
              <div className="mb-3 h-px w-8 bg-accent" aria-hidden />
              <h3 className="text-[15px] font-semibold text-ink">{item.t}</h3>
              <p className="mt-1.5 text-[14px] leading-relaxed text-ink-2">
                {item.d}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
