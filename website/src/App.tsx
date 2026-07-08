import { useCallback, useEffect, useState } from "react";
import { CheckoutNotice } from "./components/CheckoutNotice";
import { Features } from "./components/Features";
import { Footer } from "./components/Footer";
import { Header } from "./components/Header";
import { Hero } from "./components/Hero";
import { HowItWorks } from "./components/HowItWorks";
import { Pricing } from "./components/Pricing";

export default function App() {
  const [checkoutOpen, setCheckoutOpen] = useState(false);

  const openCheckout = useCallback(() => {
    setCheckoutOpen(true);
  }, []);

  const closeCheckout = useCallback(() => {
    setCheckoutOpen(false);
  }, []);

  useEffect(() => {
    if (!checkoutOpen) return;

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeCheckout();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [checkoutOpen, closeCheckout]);

  return (
    <div className="min-h-dvh bg-bg text-ink">
      <Header />
      <main>
        <Hero />
        <HowItWorks />
        <Features />
        <Pricing onBuy={openCheckout} />
      </main>
      <Footer />
      <CheckoutNotice open={checkoutOpen} onClose={closeCheckout} />
    </div>
  );
}
