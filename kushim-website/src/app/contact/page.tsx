import type { Metadata } from "next";
import { PlaceholderPage } from "@/app/_components/placeholder-page";

export const metadata: Metadata = { title: "Contact", robots: { index: false, follow: false } };

export default function ContactPage() {
  return <PlaceholderPage title="Contact" />;
}
