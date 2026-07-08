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
    title: "Tray-first, always ready",
    body: "It lives in the system tray. Open the manager for fifteen seconds, add a snippet, close it. Back to your work.",
  },
  {
    title: "Ridiculously light",
    body: "Built for speed: ~0% idle CPU, ~16 MB RAM, a few megabytes on disk. No Electron bloat. No waiting spinners.",
  },
  {
    title: "Launch at login",
    body: "Turn it on once. HyperType starts with Windows and is ready the moment you sit down.",
  },
  {
    title: "Your snippets stay yours",
    body: "Snippets live on your machine. No subscription trap for your own phrases. Buy once, expand forever.",
  },
] as const;

export function Features() {
  return (
    <section id="features" className="border-b border-sep">
      <div className="mx-auto max-w-6xl px-5 py-16 sm:px-8 sm:py-20">
        <div className="max-w-2xl">
          <p className="text-[13px] font-semibold text-accent-ink">Features</p>
          <h2 className="mt-2 font-display text-[clamp(1.75rem,3vw,2.25rem)] font-semibold tracking-tight text-ink">
            Built for people who type the same things all day
          </h2>
          <p className="mt-3 text-[16px] leading-relaxed text-ink-2">
            Support replies. Client updates. Your full address. A polite
            “thanks for waiting.” HyperType makes the boring parts of typing
            disappear.
          </p>
        </div>

        <div className="mt-12 grid gap-px overflow-hidden rounded-[10px] border border-line bg-sep sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <article
              key={f.title}
              className="bg-bg p-6 transition-colors duration-150 hover:bg-raise/50"
            >
              <div className="mb-3 h-1 w-6 rounded-full bg-accent" />
              <h3 className="font-display text-[16px] font-semibold text-ink">
                {f.title}
              </h3>
              <p className="mt-2 text-[14px] leading-relaxed text-ink-2">
                {f.body}
              </p>
            </article>
          ))}
        </div>

        {/* Benefit strip */}
        <div className="mt-10 rounded-[10px] border border-line bg-raise/30 px-6 py-6 sm:px-8">
          <div className="grid gap-6 sm:grid-cols-3">
            {[
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
            ].map((item) => (
              <div key={item.t}>
                <h3 className="text-[15px] font-semibold text-ink">{item.t}</h3>
                <p className="mt-1.5 text-[13px] leading-relaxed text-ink-2">
                  {item.d}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
