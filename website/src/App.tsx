import { useCallback, useState } from "react";
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

  return (
    <div className="min-h-dvh bg-bg text-ink">
      <a href="#main" className="skip-link">
        Skip to content
      </a>
      <Header onBuy={openCheckout} />
      <main id="main">
        <Hero onBuy={openCheckout} />
        <HowItWorks />
        <Features />
        <Pricing onBuy={openCheckout} />
      </main>
      <Footer onBuy={openCheckout} />
      <CheckoutNotice open={checkoutOpen} onClose={closeCheckout} />
    </div>
  );
}
