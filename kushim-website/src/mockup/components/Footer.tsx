"use client";

import React from 'react';
import { Link } from '@/lib/router-shim';
import { BrandMark } from './BrandMark';

const navColumns = [
  {
    heading: 'Guide',
    links: [
      { label: 'Plan du site', href: '/sitemap' },
  
    ],
  },
  {
    heading: 'Ressources',
    links: [
      { label: 'Documentation', href: '/docs' },
     
    ],
  },
  {
    heading: 'À propos',
    links: [
      { label: 'Contact', href: '/contact' },
    ],
  },
];

const legalLinks = [
  { label: 'Cookies', href: '/cookies' },
  { label: 'CGU', href: '/cgu' },
  { label: 'Politique de confidentialité', href: '/confidentialite' },
  { label: 'Mentions légales', href: '/mentions-legales' },
];

export function Footer() {
  return (
    <footer
      aria-label="Pied de page"
      className="pb-12 px-4 sm:px-6"
      style={{
        paddingTop: 'clamp(80px, 15vw, 160px)',
      }}
    >
      <div className="max-w-[1440px] mx-auto">
        {/* Main grid */}
        <div
          className="glass flex flex-col md:flex-row mb-16 rounded-[var(--radius-xl)] px-6 py-8 md:px-8"
          style={{
            gap: 'clamp(40px, 8vw, 64px)',
          }}
        >
          {/* Left: Brand block */}
          <div className="md:w-1/3 flex flex-col items-center md:items-start text-center md:text-left">
            <Link to="/" aria-label="Kushim — Accueil" className="flex min-h-[44px] items-center">
              <BrandMark variant="standard" />
            </Link>
            <span
              className="mt-2"
              style={{
                fontSize: 'clamp(13px, 2vw, 14px)',
                color: 'var(--text-tertiary)',
              }}
            >
              © 2026
            </span>
            <span
              className="mt-0.5"
              style={{
                fontSize: 'clamp(13px, 2vw, 14px)',
                color: 'var(--text-tertiary)',
              }}
            >
              Thibault Paul
            </span>
          </div>

          {/* Right: Nav columns */}
          <nav
            aria-label="Navigation secondaire"
            className="md:w-2/3 grid grid-cols-1 sm:grid-cols-3 text-center sm:text-left"
            style={{
              gap: 'clamp(32px, 5vw, 48px)',
            }}
          >
            {navColumns.map((col) => (
              <div key={col.heading}>
                <h2
                  className="mb-4"
                  style={{
                    fontSize: 'clamp(13px, 2.2vw, 14px)',
                    fontWeight: 600,
                    color: 'var(--text-primary)',
                  }}
                >
                  {col.heading}
                </h2>
                <ul className="flex flex-col" style={{ gap: 'clamp(10px, 2vw, 12px)' }}>
                  {col.links.map((link) => (
                    <li key={link.label} style={{ minHeight: '44px', display: 'flex', alignItems: 'center', justifyContent: 'center' }} className="sm:justify-start">
                      <Link
                        to={link.href}
                        className="transition-colors duration-200 no-underline flex min-h-[44px] items-center"
                        style={{
                          fontSize: 'clamp(13px, 2.2vw, 14px)',
                          fontWeight: 400,
                          color: 'var(--text-secondary)',
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.color = 'var(--text-primary)';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.color = 'var(--text-secondary)';
                        }}
                      >
                        {link.label}
                      </Link>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </nav>
        </div>

        {/* Legal row */}
        <nav
          aria-label="Liens légaux"
          className="flex flex-wrap justify-center md:justify-end"
          style={{
            gap: 'clamp(16px, 3vw, 24px)',
          }}
        >
          {legalLinks.map((link) => (
            <Link
              key={link.label}
              to={link.href}
              className="transition-colors duration-200 no-underline"
              style={{
                fontSize: 'clamp(11px, 2vw, 12px)',
                color: 'var(--text-secondary)',
                minHeight: '44px',
                display: 'flex',
                alignItems: 'center',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = 'var(--text-primary)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = 'var(--text-secondary)';
              }}
            >
              {link.label}
            </Link>
          ))}
        </nav>
      </div>
    </footer>
  );
}
