import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Cookies", robots: { index: false, follow: false } };

export default function CookiesPage() {
  return <PlaceholderPage title="Cookies" />;
}
