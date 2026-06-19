import type { Metadata } from "next";
import { PageState } from "@/app/_components/page-state";

export const metadata: Metadata = { title: "Page introuvable", robots: { index: false, follow: false } };

export default function NotFound() {
  return (
    <PageState
      title="Page introuvable"
      description="L’adresse demandée n’existe pas ou n’est plus disponible."
      showBack
    />
  );
}
