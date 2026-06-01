"use client";

import { useEffect, useState, type ReactNode } from "react";
import { BackgroundLayers } from "@/mockup/components/background/BackgroundLayers";
import { Footer } from "@/mockup/components/Footer";
import { Navbar } from "@/mockup/components/Navbar";

export function AuthShell({ children }: { children: ReactNode }) {
  const [isDark, setIsDark] = useState(true);

  useEffect(() => {
    const savedTheme = localStorage.getItem("theme");
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    setIsDark(savedTheme ? savedTheme === "dark" : prefersDark);
  }, []);

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
    <div className="min-h-screen relative" style={{ backgroundColor: "var(--canvas-bg)" }}>
      <BackgroundLayers />
      <div className="relative z-10">
        <Navbar isAuthenticated={false} onThemeToggle={toggleTheme} isDark={isDark} />
        <main style={{ paddingTop: "44px" }}>{children}</main>
        <Footer />
      </div>
    </div>
  );
}
