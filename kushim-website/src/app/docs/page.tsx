import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Documentation", robots: { index: false, follow: false } };

export default function DocsPage() {
  return <PlaceholderPage title="Documentation" />;
}
