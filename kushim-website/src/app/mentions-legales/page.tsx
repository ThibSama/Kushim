import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Mentions légales", robots: { index: false, follow: false } };

export default function MentionsLegalesPage() {
  return <PlaceholderPage title="Mentions légales" />;
}
