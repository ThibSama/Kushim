import type { Metadata } from "next";
import { HealthClient } from "./health-client";

export const metadata: Metadata = {
  title: "État des services",
  description:
    "Disponibilité en temps réel des services Kushim (site, authentification, API, traitement des portefeuilles, données de marché).",
  alternates: { canonical: "/health" },
  robots: { index: false, follow: false },
};

export default function HealthPage() {
  return <HealthClient />;
}
