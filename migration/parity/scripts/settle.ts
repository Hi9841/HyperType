/**
 * Wait until the page is visually stable for a screenshot.
 *
 * 1. Freeze infinite CSS animations at currentTime = 0 (documented freeze frame).
 * 2. Wait until no finite animation or transition is running.
 * 3. Wait two animation frames.
 */
import type { Page } from "playwright-core";

/** Documented freeze: infinite animations pause at t=0 (start of the cycle). */
export const INFINITE_ANIM_FREEZE_MS = 0;

export async function settleForScreenshot(page: Page): Promise<void> {
  await page.evaluate(async (freezeMs: number) => {
    const isInfinite = (a: Animation): boolean => {
      const iters = a.effect?.getComputedTiming().iterations;
      return iters === Infinity;
    };

    for (const a of document.getAnimations()) {
      if (!isInfinite(a)) continue;
      try {
        a.pause();
        a.currentTime = freezeMs;
      } catch {
        // ignore animations that reject pause
      }
    }

    const deadline = performance.now() + 5_000;
    while (performance.now() < deadline) {
      const finite = document
        .getAnimations()
        .filter((a) => !isInfinite(a) && a.playState !== "finished" && a.playState !== "idle");
      if (finite.length === 0) break;
      await Promise.all(
        finite.map((a) =>
          a.finished.then(
            () => undefined,
            () => undefined,
          ),
        ),
      );
    }

    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => resolve());
      });
    });
  }, INFINITE_ANIM_FREEZE_MS);
}
