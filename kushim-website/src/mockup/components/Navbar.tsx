"use client";

import React, { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "@/lib/router-shim";
import {
  LayoutDashboard,
  BarChart2,
  List,
  Compass,
  Settings,
  Sun,
  Moon,
  Menu,
  X,
  Box,
  Shield,
  CreditCard,
} from "lucide-react";
import { ENABLE_DISCOVER } from "../config/features";
import { BrandMark } from "./BrandMark";

interface NavbarProps {
  isAuthenticated?: boolean;
  onThemeToggle?: () => void;
  isDark?: boolean;
}

export function Navbar({
  isAuthenticated = false,
  onThemeToggle,
  isDark = false,
}: NavbarProps) {
  const authUrl = process.env.NEXT_PUBLIC_AUTH_URL ?? "http://localhost:3001";
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const location = useLocation();
  const navigate = useNavigate();

  const isActive = (path: string) => location.pathname === path;

  const handleAnchorClick = (e: React.MouseEvent, href: string) => {
    const hashIndex = href.indexOf("#");
    if (hashIndex === -1) return;
    const hash = href.slice(hashIndex);
    const basePath = href.slice(0, hashIndex) || "/";

    if (location.pathname === basePath) {
      e.preventDefault();
      const el = document.querySelector(hash);
      if (el) el.scrollIntoView({ behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    } else {
      e.preventDefault();
      navigate(basePath);
      setTimeout(() => {
        const el = document.querySelector(hash);
        if (el) el.scrollIntoView({ behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
      }, 100);
    }
  };

  const publicLinks = [
    { label: "Produit", href: "/#produit", icon: Box },
    { label: "Sécurité", href: "/#securite", icon: Shield },
    { label: "Tarifs", href: "/#tarifs", icon: CreditCard },
  ];

  const appLinks = [
    { label: "Tableau de bord", href: "/dashboard", icon: LayoutDashboard },
    { label: "Actifs", href: "/assets", icon: BarChart2 },
    { label: "Transactions", href: "/transactions", icon: List },
    ...(ENABLE_DISCOVER
      ? [{ label: "Découvrir", href: "/discover", icon: Compass }]
      : []),
    { label: "Paramètres", href: "/settings", icon: Settings },
  ];

  const links = isAuthenticated ? appLinks : publicLinks;

  useEffect(() => {
    if (!mobileMenuOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMobileMenuOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [mobileMenuOpen]);

  return (
    <>
      <nav
        className="fixed inset-x-0 z-50 flex justify-center px-3 sm:px-4"
        style={{ top: "clamp(16px, 3vw, 28px)" }}>
        <div
          className="glass-elevated flex max-w-full items-center rounded-[9999px]"
          style={{
            minHeight: "clamp(44px, 8vw, 48px)",
            padding: "4px",
            paddingInline: "clamp(10px, 1.6vw, 14px)",
            maxWidth: "calc(100vw - 24px)",
            gap: "clamp(8px, 1vw, 12px)",
          }}>
          <Link
            to={isAuthenticated ? "/dashboard" : "/"}
            onClick={(e: React.MouseEvent) => {
              if (!isAuthenticated && location.pathname === "/") {
                e.preventDefault();
                window.scrollTo({
                  top: 0,
                  behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
                });
              }
            }}
            aria-label="Kushim — Accueil"
            className="shrink-0 flex min-h-[44px] items-center"
            style={{
              paddingInline: "clamp(6px, 1vw, 8px)",
            }}>
            <BrandMark variant="compact" />
          </Link>

          <div
            className="hidden md:flex items-center shrink-0"
            style={{ gap: "clamp(4px, 0.6vw, 6px)" }}>
            {links.map((link) => {
              const active = isActive(link.href);
              const isAnchor = link.href.includes("#");
              return (
                <Link
                  key={link.href}
                  to={link.href}
                  onClick={
                    isAnchor
                      ? (e: React.MouseEvent) => handleAnchorClick(e, link.href)
                      : undefined
                  }
                  className="rounded-[9999px] transition-all duration-200"
                  style={{
                    fontSize: "clamp(13px, 1.6vw, 14px)",
                    fontWeight: active ? 600 : 500,
                    color: active
                      ? isDark
                        ? "#FAFAFA"
                        : "var(--text-primary)"
                      : isDark
                        ? "#A1A1AA"
                        : "var(--text-secondary)",
                    background: active
                      ? "linear-gradient(180deg, rgba(255,255,255,0.08), rgba(255,255,255,0.02)), var(--surface-2-bg)"
                      : "transparent",
                    border: active
                      ? "1px solid var(--glass-border)"
                      : "1px solid transparent",
                    boxShadow: active ? "var(--glass-highlight)" : "none",
                    minHeight: "44px",
                    display: "flex",
                    alignItems: "center",
                    padding: "0 14px",
                    whiteSpace: "nowrap",
                  }}
                  onMouseEnter={(e) => {
                    if (!active) {
                      e.currentTarget.style.background = isDark
                        ? "linear-gradient(180deg, rgba(255,255,255,0.06), rgba(255,255,255,0.015)), rgba(255, 255, 255, 0.04)"
                        : "linear-gradient(180deg, rgba(255,255,255,0.08), rgba(255,255,255,0.02)), rgba(255, 255, 255, 0.24)";
                      e.currentTarget.style.color = isDark
                        ? "#FAFAFA"
                        : "var(--text-primary)";
                      e.currentTarget.style.borderColor = "var(--glass-border)";
                    }
                  }}
                  onMouseLeave={(e) => {
                    if (!active) {
                      e.currentTarget.style.background = "transparent";
                      e.currentTarget.style.borderColor = "transparent";
                      e.currentTarget.style.color = isDark
                        ? "#A1A1AA"
                        : "var(--text-secondary)";
                    }
                  }}>
                  {link.label}
                </Link>
              );
            })}
          </div>

          <div
            className="flex items-center shrink-0"
            style={{ gap: "clamp(2px, 0.75vw, 4px)" }}>
            <button
              type="button"
              onClick={onThemeToggle}
              className="rounded-full flex items-center justify-center shrink-0 transition-all duration-200"
              style={{
                color: "var(--text-secondary)",
                minWidth: "44px",
                minHeight: "44px",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background =
                  "linear-gradient(180deg, rgba(255,255,255,0.06), rgba(255,255,255,0.02)), var(--surface-2-bg)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = "transparent";
              }}
              aria-label={isDark ? "Activer le thème clair" : "Activer le thème sombre"}>
              {isDark ? <Sun size={16} aria-hidden="true" /> : <Moon size={16} aria-hidden="true" />}
            </button>

            <span
              className="rounded-full flex items-center justify-center shrink-0 transition-all duration-200"
              style={{
                color: "var(--text-secondary)",
                fontSize: "clamp(11px, 2vw, 12px)",
                fontWeight: 600,
                minWidth: "44px",
                minHeight: "44px",
              }}
              aria-label="Langue actuelle : français">
              FR
            </span>

            <div
              className="hidden md:flex items-center shrink-0"
              style={{ gap: "clamp(4px, 1vw, 6px)" }}>
              <div
                className="mx-1 shrink-0"
                style={{
                  width: "1px",
                  height: "20px",
                  background: isDark
                    ? "rgba(255, 255, 255, 0.10)"
                    : "rgba(0, 0, 0, 0.08)",
                }}
              />

              {!isAuthenticated ? (
                <>
                  <Link
                    to={`${authUrl}/connexion`}
                    className="px-3.5 py-1.5 rounded-[9999px] transition-all duration-200"
                    style={{
                      fontSize: "clamp(13px, 2vw, 14px)",
                      fontWeight: 500,
                      color: "var(--text-secondary)",
                      minHeight: "44px",
                      whiteSpace: "nowrap",
                      display: "flex",
                      alignItems: "center",
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.color = "var(--text-primary)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.color = "var(--text-secondary)";
                    }}>
                    Se connecter
                  </Link>
                  <Link
                    to={`${authUrl}/inscription`}
                    className="rounded-[9999px] transition-opacity duration-200 hover:opacity-90 flex items-center justify-center shrink-0"
                    style={{
                      background: isDark ? "#FAFAFA" : "#09090B",
                      color: isDark ? "#09090B" : "#FFFFFF",
                      fontSize: "clamp(13px, 2vw, 14px)",
                      fontWeight: 600,
                      minHeight: "44px",
                      whiteSpace: "nowrap",
                      padding: "0 clamp(16px, 3vw, 20px)",
                    }}>
                    Commencer
                  </Link>
                </>
              ) : (
                <div
                  className="glass-field flex items-center px-1 pr-3 rounded-[9999px] shrink-0"
                  style={{
                    minHeight: "36px",
                    gap: "clamp(6px, 1vw, 8px)",
                  }}>
                  <div
                    className="rounded-full flex items-center justify-center"
                    style={{
                      background: "var(--color-accent)",
                      color: "white",
                      fontSize: "clamp(11px, 2vw, 12px)",
                      fontWeight: 700,
                      width: "clamp(28px, 5vw, 32px)",
                      height: "clamp(28px, 5vw, 32px)",
                    }}>
                    U
                  </div>
                  <span
                    style={{
                      fontSize: "clamp(13px, 2vw, 14px)",
                      fontWeight: 500,
                      color: "var(--text-primary)",
                      whiteSpace: "nowrap",
                    }}>
                    Utilisateur
                  </span>
                </div>
              )}
            </div>

            <button
              type="button"
              onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
              className="md:hidden rounded-full flex items-center justify-center transition-all duration-200"
              style={{
                color: "var(--text-secondary)",
                minWidth: "44px",
                minHeight: "44px",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background =
                  "linear-gradient(180deg, rgba(255,255,255,0.06), rgba(255,255,255,0.02)), var(--surface-2-bg)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = "transparent";
              }}
              aria-label="Ouvrir le menu"
              aria-expanded={mobileMenuOpen}
              aria-controls="mobile-navigation">
              <Menu size={20} aria-hidden="true" />
            </button>
          </div>
        </div>
      </nav>

      {mobileMenuOpen && (
        <>
          <div
            className="fixed inset-0 z-40"
            style={{ background: "rgba(0, 0, 0, 0.30)", backdropFilter: "blur(8px)" }}
            onClick={() => setMobileMenuOpen(false)}
            aria-hidden="true"
          />

          <div
            id="mobile-navigation"
            role="dialog"
            aria-modal="true"
            aria-label="Navigation principale"
            className="glass-strong fixed top-0 left-0 right-0 z-50 rounded-b-[var(--radius-xl)]"
            style={{
              padding: "clamp(16px, 4vw, 20px)",
            }}>
            <div
              className="flex justify-between items-center"
              style={{ marginBottom: "clamp(16px, 3vw, 20px)" }}>
              <BrandMark variant="compact" />
              <button
                type="button"
                onClick={() => setMobileMenuOpen(false)}
                className="rounded-full flex items-center justify-center"
                style={{
                  minWidth: "44px",
                  minHeight: "44px",
                  color: "var(--text-primary)",
                }}
                aria-label="Fermer le menu">
                <X size={20} aria-hidden="true" />
              </button>
            </div>

            <div
              className="flex flex-col"
              style={{ gap: "clamp(8px, 2vw, 10px)" }}>
              {links.map((link) => {
                const Icon = link.icon;
                const isAnchor = link.href.includes("#");
                return (
                  <Link
                    key={link.href}
                    to={link.href}
                    onClick={(e: React.MouseEvent) => {
                      setMobileMenuOpen(false);
                      if (isAnchor) handleAnchorClick(e, link.href);
                    }}
                    className="flex items-center rounded-lg"
                    style={{
                      color: "var(--text-primary)",
                      background: isActive(link.href)
                        ? "var(--surface-2-bg)"
                        : "transparent",
                      gap: "clamp(10px, 2vw, 12px)",
                      padding:
                        "clamp(12px, 2.5vw, 14px) clamp(14px, 3vw, 16px)",
                      minHeight: "44px",
                      fontSize: "clamp(14px, 2.5vw, 15px)",
                    }}>
                    <Icon size={20} aria-hidden="true" />
                    {link.label}
                  </Link>
                );
              })}

              {!isAuthenticated && (
                <div
                  className="flex flex-col mt-4"
                  style={{ gap: "clamp(10px, 2vw, 12px)" }}>
                  <Link
                    to={`${authUrl}/connexion`}
                    onClick={() => setMobileMenuOpen(false)}
                    className="text-center rounded-[9999px]"
                    style={{
                      border: "1px solid var(--surface-1-border)",
                      color: "var(--text-primary)",
                      padding:
                        "clamp(12px, 2.5vw, 14px) clamp(14px, 3vw, 16px)",
                      minHeight: "44px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "clamp(14px, 2.5vw, 15px)",
                    }}>
                    Se connecter
                  </Link>
                  <Link
                    to={`${authUrl}/inscription`}
                    onClick={() => setMobileMenuOpen(false)}
                    className="text-center rounded-[9999px]"
                    style={{
                      background: "var(--color-cta-bg)",
                      color: "var(--color-cta-text)",
                      fontWeight: 600,
                      padding:
                        "clamp(12px, 2.5vw, 14px) clamp(14px, 3vw, 16px)",
                      minHeight: "44px",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "clamp(14px, 2.5vw, 15px)",
                    }}>
                    Commencer
                  </Link>
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </>
  );
}
