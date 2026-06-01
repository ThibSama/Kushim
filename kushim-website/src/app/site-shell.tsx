"use client";

import { useEffect, useState, type ReactNode } from "react";
import { BackgroundLayers } from "@/mockup/components/background/BackgroundLayers";
import { Footer } from "@/mockup/components/Footer";
import { Navbar } from "@/mockup/components/Navbar";

export function SiteShell({ children }: { children: ReactNode }) {
  const [isDark, setIsDark] = useState(true);

  useEffect(() => {
    const savedTheme = localStorage.getItem("theme");
    const nextIsDark =
      savedTheme === "dark" ||
      (savedTheme !== "light" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    setIsDark(nextIsDark);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", isDark);
  }, [isDark]);

  const toggleTheme = () => {
    const next = !isDark;
    setIsDark(next);
    localStorage.setItem("theme", next ? "dark" : "light");
  };

  return (
    <div className="relative min-h-screen" style={{ backgroundColor: "var(--canvas-bg)" }}>
      <BackgroundLayers />
      <div className="relative z-10">
        <Navbar isAuthenticated={false} isDark={isDark} onThemeToggle={toggleTheme} />
        <main style={{ paddingTop: "44px" }}>{children}</main>
        <Footer />
      </div>
    </div>
  );
}
