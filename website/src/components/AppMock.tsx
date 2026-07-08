const snippets = [
  { trigger: "gm", expansion: "Good morning," },
  { trigger: "addr", expansion: "123 Market St, Suite 400" },
  { trigger: "sig", expansion: "Best regards,\nAlex Rivera" },
  { trigger: "brb", expansion: "Be right back" },
  { trigger: "omw", expansion: "On my way!" },
] as const;

export function AppMock() {
  return (
    <div
      className="relative mx-auto w-full max-w-[300px] select-none"
      aria-hidden="true"
    >
      <div className="absolute -inset-8 rounded-full bg-accent/15 blur-3xl" />

      <div className="relative overflow-hidden rounded-[14px] border border-line-2 bg-bg shadow-[0_40px_80px_-20px_rgba(0,0,0,0.9)]">
        {/* Titlebar */}
        <div className="flex h-11 items-center justify-between border-b border-sep px-3.5">
          <div className="flex items-center gap-2">
            <img
              src="/logo-mark.png"
              alt=""
              width={16}
              height={16}
              className="h-4 w-4"
            />
            <span className="font-display text-[12px] font-semibold text-ink">
              HyperType
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="h-2.5 w-2.5 rounded-full bg-line-2" />
            <span className="h-2.5 w-2.5 rounded-full bg-danger/80" />
          </div>
        </div>

        {/* Settings */}
        <div className="border-b border-sep px-3.5 py-3">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-[12px] font-semibold text-ink">
                Text Expansion
              </div>
              <div className="mt-0.5 text-[11px] text-accent-ink">
                Running · 5 snippets
              </div>
            </div>
            <div className="relative h-[18px] w-8 rounded-full bg-accent">
              <span className="absolute top-[2px] right-[2px] h-[14px] w-[14px] rounded-full bg-ink" />
            </div>
          </div>
        </div>

        {/* Composer */}
        <div className="space-y-2 border-b border-sep px-3.5 py-3">
          <div className="flex rounded-[7px] border border-line bg-field p-0.5">
            <div className="flex-1 rounded-[6px] bg-raise py-1 text-center text-[11px] font-semibold text-ink">
              Text
            </div>
            <div className="flex-1 py-1 text-center text-[11px] text-ink-3">
              Shortcut
            </div>
          </div>
          <div className="rounded-[7px] border border-line bg-field px-2.5 py-1.5 font-data text-[11px] text-ink-3">
            trigger
          </div>
          <div className="rounded-[7px] border border-line bg-field px-2.5 py-1.5 text-[11px] text-ink-3">
            expansion
          </div>
          <div className="rounded-[7px] bg-accent py-1.5 text-center text-[12px] font-semibold text-ink">
            Add
          </div>
        </div>

        {/* Snippets */}
        <div className="px-3.5 py-2.5">
          <div className="mb-1.5 flex items-center gap-2">
            <span className="text-[12px] font-semibold text-ink-2">
              Snippets
            </span>
            <span className="rounded-full bg-raise px-1.5 py-px font-data text-[10px] text-ink-3">
              5
            </span>
          </div>
          <ul>
            {snippets.map((s, i) => (
              <li
                key={s.trigger}
                className={`flex items-center gap-2 py-2 ${
                  i < snippets.length - 1 ? "border-b border-sep" : ""
                }`}
              >
                <span className="w-[52px] shrink-0 font-data text-[11px] font-semibold text-accent-ink">
                  {s.trigger}
                </span>
                <span className="truncate text-[11px] text-ink-2">
                  {s.expansion.split("\n")[0]}
                </span>
              </li>
            ))}
          </ul>
        </div>

        {/* Live typing demo strip */}
        <div className="border-t border-sep bg-raise/60 px-3.5 py-2.5">
          <div className="text-[10px] font-semibold tracking-wide text-ink-3">
            Anywhere on Windows
          </div>
          <div className="mt-1 font-data text-[12px] text-ink">
            <span className="text-ink-3">You type </span>
            <span className="text-accent-ink">gm</span>
            <span className="text-ink-3"> → </span>
            <span>Good morning,</span>
          </div>
        </div>
      </div>
    </div>
  );
}
