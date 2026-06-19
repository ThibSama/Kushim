import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Politique de confidentialité", robots: { index: false, follow: false } };

export default function ConfidentialitePage() {
  return <PlaceholderPage title="Politique de confidentialité" />;
}
