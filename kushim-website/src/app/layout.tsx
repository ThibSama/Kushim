import type { Metadata } from "next";
import { getSiteUrl } from "@/lib/site-url";
import "./globals.css";
import { SiteShell } from "./site-shell";

const siteUrl = getSiteUrl();
const title = "Kushim — Suivi patrimonial privé";
const description = "Kushim centralise le suivi de vos portefeuilles et de vos positions dans une interface claire et indépendante.";

export const metadata: Metadata = {
  metadataBase: siteUrl,
  title: {
    default: title,
    template: "%s | Kushim",
  },
  description,
  applicationName: "Kushim",
  alternates: { canonical: "/" },
  openGraph: {
    title,
    description,
    type: "website",
    locale: "fr_FR",
    siteName: "Kushim",
    url: "/",
  },
  twitter: {
    card: "summary",
    title,
    description,
  },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="fr" suppressHydrationWarning>
      <body>
        <SiteShell>{children}</SiteShell>
      </body>
    </html>
  );
}
