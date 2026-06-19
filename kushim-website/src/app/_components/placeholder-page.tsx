"use client";

import Link from "next/link";
import { ArrowLeft, Home } from "lucide-react";
import { Button } from "@/mockup/components/Button";
import { Card } from "@/mockup/components/Card";

export function PlaceholderPage({ title }: { title: string }) {
  const goBack = () => {
    try {
      const hasUsefulHistory =
        window.history.length > 1 &&
        Boolean(document.referrer) &&
        new URL(document.referrer).origin === window.location.origin;

      if (hasUsefulHistory) {
        window.history.back();
        return;
      }
    } catch {
      // A malformed or unavailable referrer is equivalent to direct navigation.
    }

    window.location.assign("/");
  };

  return (
    <section className="px-4 sm:px-6 py-20 sm:py-28">
      <div className="mx-auto max-w-[680px]">
        <Card level={1}>
          <div className="text-center py-6 sm:py-10 px-2 sm:px-6">
            <h1
              style={{
                color: "var(--text-primary)",
                fontSize: "clamp(28px, 6vw, 42px)",
                fontWeight: 800,
                lineHeight: 1.15,
              }}
            >
              {title}
            </h1>
            <p
              className="mt-4"
              style={{ color: "var(--text-secondary)", fontSize: "clamp(15px, 2.5vw, 17px)" }}
            >
              Ce contenu sera disponible prochainement.
            </p>
            <div className="mt-8 flex flex-col sm:flex-row items-stretch sm:items-center justify-center gap-3">
              <Button variant="secondary" icon={ArrowLeft} onClick={goBack}>
                Retour
              </Button>
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
