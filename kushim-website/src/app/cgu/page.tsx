import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Conditions générales d’utilisation", robots: { index: false, follow: false } };

export default function CguPage() {
  return <PlaceholderPage title="Conditions générales d’utilisation" />;
}
