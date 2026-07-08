import { useEffect, useState } from "react";

const snippets = [
  { trigger: "gm", expansion: "Good morning," },
  { trigger: "addr", expansion: "123 Market St, Suite 400" },
  { trigger: "sig", expansion: "Best regards,\nAlex Rivera" },
  { trigger: "brb", expansion: "Be right back" },
  { trigger: "omw", expansion: "On my way!" },
] as const;

const demo = {
  trigger: "gm",
  expansion: "Good morning,",
} as const;

type Phase = "idle" | "typing" | "hold" | "expanded" | "pause";

function useTypingDemo(reduced: boolean) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [typed, setTyped] = useState("");
  const [flash, setFlash] = useState(false);

  useEffect(() => {
    if (reduced) {
      setPhase("expanded");
      setTyped(demo.expansion);
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;

    const wait = (ms: number) =>
      new Promise<void>((resolve) => {
        timer = setTimeout(resolve, ms);
      });

    const run = async () => {
      while (!cancelled) {
        setPhase("idle");
        setTyped("");
        setFlash(false);
        await wait(900);

        setPhase("typing");
        for (let i = 1; i <= demo.trigger.length; i++) {
          if (cancelled) return;
          setTyped(demo.trigger.slice(0, i));
          await wait(140);
        }

        setPhase("hold");
        await wait(380);

        setPhase("expanded");
        setTyped(demo.expansion);
        setFlash(true);
        await wait(520);
        setFlash(false);
        await wait(1800);

        setPhase("pause");
        await wait(700);
      }
    };

    void run();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [reduced]);

  return { phase, typed, flash };
}

export function AppMock() {
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const apply = () => setReduced(mq.matches);
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, []);

  const { phase, typed, flash } = useTypingDemo(reduced);
  const showCaret = phase === "typing" || phase === "hold" || phase === "idle";

  return (
    <div className="relative mx-auto w-full max-w-[300px] select-none">
      <div
        className="pointer-events-none absolute -inset-10 rounded-full bg-accent/15 blur-3xl"
        aria-hidden
      />

      <div className="mock-window relative" aria-hidden="true">
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
              <div className="mt-0.5 flex items-center gap-1.5 text-[11px] text-accent-ink">
                <span className="status-dot" />
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
            <span className="text-[12px] font-semibold text-ink-2">Snippets</span>
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
        <div className="border-t border-sep bg-raise/70 px-3.5 py-3">
          <div className="flex items-center justify-between gap-2">
            <div className="text-[10px] font-semibold tracking-wide text-ink-3">
              Live in any app
            </div>
            <div className="font-data text-[10px] text-ink-3">
              {phase === "expanded" ? "expanded" : "typing"}
            </div>
          </div>
          <div
            className={`mt-2 rounded-[7px] border border-line bg-field px-2.5 py-2 font-data text-[12px] text-ink ${
              flash ? "flash-expand" : ""
            }`}
          >
            <span
              className={
                phase === "expanded" || phase === "pause"
                  ? "text-ink"
                  : "text-accent-ink"
              }
            >
              {typed || "\u00a0"}
            </span>
            {showCaret ? <span className="caret" /> : null}
          </div>
          <div className="mt-2 flex items-center gap-1.5 text-[10px] text-ink-3">
            <span className="kbd">gm</span>
            <span>→</span>
            <span className="text-ink-2">Good morning,</span>
          </div>
        </div>
      </div>

      <p className="sr-only">
        Demo of HyperType expanding the trigger gm into Good morning.
      </p>
    </div>
  );
}
