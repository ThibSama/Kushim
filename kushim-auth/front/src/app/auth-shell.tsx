"use client";

import { useEffect, useState, type ReactNode } from "react";
import { BackgroundLayers } from "@/mockup/components/background/BackgroundLayers";
import { Footer } from "@/mockup/components/Footer";
import { Navbar } from "@/mockup/components/Navbar";
import { I18nProvider } from "@/i18n/context";

export function AuthShell({ children }: { children: ReactNode }) {
  // Lazy initializer reads the value the pre-hydration script in layout.tsx
  // already applied, so React's first render matches the DOM and we don't
  // briefly overwrite the resolved theme with a hard-coded default.
  const [isDark, setIsDark] = useState(() => {
    if (typeof document === "undefined") return true;
    return document.documentElement.classList.contains("dark");
  });

  useEffect(() => {
    document.documentElement.classList.toggle("dark", isDark);
  }, [isDark]);

  const toggleTheme = () => {
    setIsDark((current) => {
      const next = !current;
      localStorage.setItem("theme", next ? "dark" : "light");
      return next;
    });
  };

  return (
    <I18nProvider>
      <div className="min-h-screen relative" style={{ backgroundColor: "var(--canvas-bg)" }}>
        <BackgroundLayers />
        <div className="relative z-10">
          <Navbar isAuthenticated={false} onThemeToggle={toggleTheme} isDark={isDark} />
          <main style={{ paddingTop: "44px" }}>{children}</main>
          <Footer />
        </div>
      </div>
    </I18nProvider>
  );
}
