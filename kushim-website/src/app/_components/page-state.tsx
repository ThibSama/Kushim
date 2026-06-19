"use client";

import Link from "next/link";
import { ArrowLeft, Home, RotateCcw } from "lucide-react";
import { Button } from "@/mockup/components/Button";
import { Card } from "@/mockup/components/Card";

type PageStateProps = {
  title: string;
  description: string;
  onRetry?: () => void;
  showBack?: boolean;
};

export function PageState({ title, description, onRetry, showBack = false }: PageStateProps) {
  const goBack = () => {
    if (window.history.length > 1) {
      window.history.back();
      return;
    }
    window.location.assign("/");
  };

  return (
    <section className="px-4 sm:px-6 py-20 sm:py-28">
      <div className="mx-auto max-w-[680px]">
        <Card level={1}>
          <div className="text-center py-6 sm:py-10 px-2 sm:px-6">
            <h1 style={{ color: "var(--text-primary)", fontSize: "clamp(28px, 6vw, 42px)", fontWeight: 800, lineHeight: 1.15 }}>
              {title}
            </h1>
            <p className="mt-4" style={{ color: "var(--text-secondary)", fontSize: "clamp(15px, 2.5vw, 17px)", lineHeight: 1.6 }}>
              {description}
            </p>
            <div className="mt-8 flex flex-col sm:flex-row items-stretch sm:items-center justify-center gap-3">
              {showBack && (
                <Button variant="secondary" icon={ArrowLeft} onClick={goBack}>
                  Retour
                </Button>
              )}
              {onRetry && (
                <Button variant="primary" icon={RotateCcw} onClick={onRetry}>
                  Réessayer
                </Button>
              )}
              <Link
                href="/"
                className="glass-interactive rounded-[9999px] flex min-h-[44px] items-center justify-center gap-2 px-6 py-3"
                style={{ color: "var(--text-primary)", border: "1px solid var(--surface-1-border)" }}
              >
                <Home size={16} aria-hidden="true" />
                Accueil
              </Link>
            </div>
          </div>
        </Card>
      </div>
    </section>
  );
}
