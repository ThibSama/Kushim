"use client";

import { useEffect } from "react";
import { PageState } from "@/app/_components/page-state";

export default function ErrorPage({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => {
    // Intentionally keep runtime details out of the UI and application logs.
    void error;
  }, [error]);

  return (
    <PageState
      title="Un problème est survenu"
      description="La page n’a pas pu être affichée. Vous pouvez réessayer ou revenir à l’accueil."
      onRetry={reset}
    />
  );
}
