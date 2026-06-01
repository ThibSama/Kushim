import React, { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { BackgroundLayers } from "./components/background/BackgroundLayers";
import { Footer } from "./components/Footer";
import { Navbar } from "./components/Navbar";

export function Root() {
  const [isDark, setIsDark] = useState(() => {
    const savedTheme = localStorage.getItem("theme");
    if (savedTheme === "dark") return true;
    if (savedTheme === "light") return false;
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  });

  useEffect(() => {
    document.documentElement.classList.toggle("dark", isDark);
  }, [isDark]);

  const toggleTheme = () => {
    const next = !isDark;
    setIsDark(next);
    localStorage.setItem("theme", next ? "dark" : "light");
  };

  return (
    <div className="min-h-screen relative" style={{ backgroundColor: "var(--canvas-bg)" }}>
      <BackgroundLayers />
      <div className="relative z-10">
        <Navbar isAuthenticated onThemeToggle={toggleTheme} isDark={isDark} />
        <main style={{ paddingTop: "44px" }}>
          <Outlet />
        </main>
        <Footer />
      </div>
    </div>
  );
}
