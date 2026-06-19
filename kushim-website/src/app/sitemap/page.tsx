import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Plan du site", robots: { index: false, follow: false } };

export default function SitemapPage() {
  return <PlaceholderPage title="Plan du site" />;
}
