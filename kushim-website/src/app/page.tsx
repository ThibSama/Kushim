import { Landing } from "@/mockup/pages/Landing";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Kushim - Suivi patrimonial prive",
  description: "Kushim est l'outil de suivi patrimonial independant. Multi-actifs. Transparent. Sans compromis.",
};

export default function HomePage() {
  return <Landing />;
}
