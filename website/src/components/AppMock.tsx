import { useEffect, useState } from "react";

const demo = {
  trigger: "gm",
  expansion: "Good morning,",
} as const;

type Phase = "idle" | "typing" | "hold" | "expanded" | "pause";

function useTypingDemo(reduced: boolean) {
  const [phase, setPhase] = useState<Phase>(reduced ? "expanded" : "idle");
  const [typed, setTyped] = useState(reduced ? demo.expansion : "");
  const [flash, setFlash] = useState(false);

  useEffect(() => {
    if (reduced) {
      setPhase("expanded");
      setTyped(demo.expansion);
      setFlash(false);
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
        await wait(360);

        setPhase("expanded");
        setTyped(demo.expansion);
        setFlash(true);
        await wait(480);
        setFlash(false);
        await wait(1700);

        setPhase("pause");
        await wait(650);
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
    <figure className="relative mx-auto w-full max-w-[320px]">
      <div
        className="pointer-events-none absolute -inset-8 rounded-full bg-accent/12 blur-3xl"
        aria-hidden
      />

      <div className="mock-frame relative">
        <img
          src="/app-screenshot.png"
          alt="HyperType tray manager: Text Expansion on with 7 snippets active, insert method Auto, snippet composer, and triggers like gm expanding to full phrases."
          width={480}
          height={854}
          decoding="async"
          fetchPriority="high"
        />
      </div>

      <figcaption className="mt-4 rounded-[10px] border border-line bg-raise/50 px-3.5 py-3">
        <div className="flex items-center justify-between gap-2">
          <span className="text-[11px] font-semibold text-ink-2">
            Expansion in any Windows app
          </span>
          <span className="font-data text-[10px] text-ink-3">
            {phase === "expanded" || phase === "pause" ? "expanded" : "typing"}
          </span>
        </div>
        <div
          className={`mt-2 rounded-[7px] border border-line bg-field px-2.5 py-2 font-data text-[13px] text-ink ${
            flash ? "flash-expand" : ""
          }`}
          aria-live="polite"
          aria-atomic="true"
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
          {showCaret ? <span className="caret" aria-hidden /> : null}
        </div>
        <div className="mt-2 flex items-center gap-1.5 text-[11px] text-ink-3">
          <span className="kbd">gm</span>
          <span aria-hidden>→</span>
          <span className="text-ink-2">Good morning,</span>
        </div>
      </figcaption>
    </figure>
  );
}
