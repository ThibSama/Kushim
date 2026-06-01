"use client";

import NextLink from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useCallback, useEffect, type AnchorHTMLAttributes, type ReactNode } from "react";

type LinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & {
  to: string;
  children: ReactNode;
};

export function Link({ to, children, ...props }: LinkProps) {
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
