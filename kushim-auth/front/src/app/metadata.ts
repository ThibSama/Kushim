import type { Metadata } from "next";

export function pageMetadata(title: string, description: string, path = "/"): Metadata {
  return {
    title,
    description,
    alternates: { canonical: path },
    openGraph: {
      title,
      description,
      siteName: "Kushim Auth",
      locale: "fr_FR",
      type: "website",
    },
    twitter: {
      card: "summary",
      title,
      description,
    },
  };
}
