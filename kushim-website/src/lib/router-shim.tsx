"use client";

import NextLink from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useCallback, useEffect, type AnchorHTMLAttributes, type ReactNode } from "react";

type LinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & {
  to: string;
  children: ReactNode;
};

function isExternalUrl(to: string): boolean {
  // Absolute cross-origin targets (e.g. http://auth.kushim.localhost/connexion)
  // and hash-only anchors must use real browser navigation, not the Next.js
  // client router (which is for internal app routes only).
  return /^https?:\/\//i.test(to) || to.startsWith("#");
}

export function Link({ to, children, ...props }: LinkProps) {
  if (isExternalUrl(to)) {
    return (
      <a href={to} {...props}>
        {children}
      </a>
    );
  }

  return (
    <NextLink href={to} {...props}>
      {children}
    </NextLink>
  );
}

export function useNavigate() {
  const router = useRouter();

  return useCallback((to: string, options?: { replace?: boolean }) => {
    if (options?.replace) {
      router.replace(to);
      return;
    }
    router.push(to);
  }, [router]);
}

export function useLocation() {
  const pathname = usePathname();

  return {
    pathname,
    search: typeof window === "undefined" ? "" : window.location.search,
  };
}

export function Navigate({ to, replace = false }: { to: string; replace?: boolean }) {
  const navigate = useNavigate();

  useEffect(() => {
    navigate(to, { replace });
  }, [navigate, replace, to]);

  return null;
}
