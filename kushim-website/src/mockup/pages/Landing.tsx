"use client";

import React, { useEffect, useRef, useState } from 'react';
import { Link } from '@/lib/router-shim';
import { Terminal, Layers, Calculator, Shield, Check, Lock, ShieldCheck } from 'lucide-react';
import { Card } from '../components/Card';
import { Button } from '../components/Button';
import { Badge } from '../components/Badge';
import { AreaChart, Area, PieChart, Pie, Cell } from 'recharts';

// Smooth organic curve data for the hero chart
const perfData = [
  2, 3, 2.5, 4, 3.8, 5, 6, 5.5, 7, 8, 7.2, 9, 10, 9.5, 11, 12.5, 11.8,
  13, 14, 13.2, 15, 14.5, 16, 17, 16.5, 18, 19, 18.2, 20, 21, 20.5, 22,
  23.5, 22.8, 24, 25, 24.2, 26, 27, 26.5, 28, 29.5, 28.8, 30, 31,
].map((v) => ({ v }));

const allocData = [
  { name: 'Actions', value: 42, color: '#60A5FA' },
  { name: 'Crypto', value: 28, color: '#A78BFA' },
  { name: 'ETF', value: 20, color: '#FBBF24' },
  { name: 'Autre', value: 10, color: '#71717A' },
];

function HeroCard({ delay = 0, children }: { delay?: number; children: React.ReactNode }) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const t = setTimeout(() => setVisible(true), delay + 100);
    return () => clearTimeout(t);
  }, [delay]);

  return (
    <div
      className="glass glass-hover text-left"
      style={{
        borderRadius: 'clamp(16px, 3vw, 20px)',
        padding: 'clamp(16px, 3vw, 24px)',
        opacity: visible ? 1 : 0,
        transform: visible ? 'translateY(0)' : 'translateY(12px)',
        transition: 'opacity 0.6s ease, transform 0.6s ease',
      }}
    >
      {children}
    </div>
  );
}

function useMeasuredChart() {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const update = () => {
      const rect = element.getBoundingClientRect();
      setSize({
        width: Math.max(0, Math.floor(rect.width)),
        height: Math.max(0, Math.floor(rect.height)),
      });
    };

    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return { ref, size };
}

export function Landing() {
  const authUrl = process.env.NEXT_PUBLIC_AUTH_URL ?? 'http://localhost:3001';
  const [chartsReady, setChartsReady] = useState(false);
  const perfChart = useMeasuredChart();
  const allocChart = useMeasuredChart();

  useEffect(() => {
    setChartsReady(true);
  }, []);

  return (
    <div className="relative overflow-x-hidden">
      {/* Hero Section */}
      <section
        className="px-4 sm:px-6"
        style={{
          paddingTop: 'clamp(80px, 15vw, 120px)',
          paddingBottom: 'clamp(64px, 12vw, 128px)',
        }}
      >
        <div className="max-w-[1440px] mx-auto text-center">
          <Badge variant="info" className="mb-4">
            Alpha
          </Badge>

          <h1
            className="mb-4 max-w-[90vw] sm:max-w-[600px] md:max-w-[700px] lg:max-w-[800px] mx-auto px-4"
            style={{
              fontSize: 'clamp(32px, 7vw, 64px)',
              fontWeight: 800,
              lineHeight: '1.05',
              letterSpacing: '-0.02em',
              color: 'var(--text-primary)',
            }}
          >
            Votre patrimoine. Votre contrôle.
          </h1>

          <p
            className="mb-6 max-w-[90vw] sm:max-w-[500px] md:max-w-[600px] mx-auto px-4"
            style={{
              fontSize: 'clamp(15px, 3vw, 18px)',
              lineHeight: '1.6',
              color: 'var(--text-secondary)',
            }}
          >
            Kushim est l'outil de suivi patrimonial indépendant. Multi-actifs. Transparent. Sans compromis.
          </p>

          <div
            className="flex flex-col sm:flex-row items-center justify-center mb-12 sm:mb-16 px-4"
            style={{
              gap: 'clamp(12px, 2vw, 16px)',
            }}
          >
            <Link to={`${authUrl}/inscription`} className="w-full sm:w-auto">
              <Button variant="primary" icon={Terminal} className="w-full sm:w-auto min-h-[44px]">
                Ouvrir mon portefeuille
              </Button>
            </Link>
            <a
              href="#securite"
              onClick={(e) => {
                e.preventDefault();
                document.querySelector('#securite')?.scrollIntoView({ behavior: 'smooth' });
              }}
              className="w-full sm:w-auto"
            >
              <Button variant="ghost" className="w-full sm:w-auto min-h-[44px]">
                Notre approche sécurité
              </Button>
            </a>
          </div>

          {/* Dashboard preview */}
          <div className="max-w-[1100px] mx-auto grid grid-cols-1 lg:grid-cols-[1fr_0.48fr] gap-4 sm:gap-6 px-4">
            {/* Left: Macro Performance */}
            <HeroCard delay={0}>
              <div className="flex flex-col h-full" style={{ minHeight: 'clamp(240px, 35vw, 280px)' }}>
                <span style={{ fontSize: 'clamp(12px, 2vw, 13px)', fontWeight: 500, color: 'var(--text-secondary)' }}>
                  Macro Performance
                </span>
                <div ref={perfChart.ref} className="flex-1 mt-4 mb-4" style={{ minHeight: 'clamp(120px, 20vw, 140px)' }}>
                  {chartsReady && perfChart.size.width > 0 && perfChart.size.height > 0 && (
                    <AreaChart data={perfData} width={perfChart.size.width} height={perfChart.size.height}>
                      <defs>
                        <linearGradient id="heroGainFill" x1="0" y1="0" x2="0" y2="1">
                          <stop offset="0%" stopColor="var(--color-gain)" stopOpacity={0.10} />
                          <stop offset="100%" stopColor="var(--color-gain)" stopOpacity={0.01} />
                        </linearGradient>
                      </defs>
                      <Area
                        type="monotone"
                        dataKey="v"
                        stroke="var(--color-gain)"
                        strokeWidth={2}
                        fill="url(#heroGainFill)"
                        dot={false}
                        isAnimationActive={true}
                        animationDuration={800}
                      />
                    </AreaChart>
                  )}
                </div>
                <div className="flex items-end justify-between">
                  <span
                    style={{
                      fontFamily: "'JetBrains Mono', monospace",
                      fontSize: 'clamp(22px, 4vw, 28px)',
                      fontWeight: 700,
                      color: 'var(--color-gain)',
                      fontVariantNumeric: 'tabular-nums',
                    }}
                  >
                    +14.2%
                  </span>
                  <span
                    className="inline-block rounded-full"
                    style={{
                      width: '6px',
                      height: '6px',
                      background: 'var(--color-gain)',
                      marginBottom: '8px',
                    }}
                  />
                </div>
              </div>
            </HeroCard>

            {/* Right column */}
            <div className="flex flex-col gap-4 sm:gap-6">
              {/* Allocation */}
              <HeroCard delay={120}>
                <span style={{ fontSize: 'clamp(12px, 2vw, 13px)', fontWeight: 500, color: 'var(--text-secondary)' }}>
                  Allocation
                </span>
                <div ref={allocChart.ref} className="flex justify-center mt-3" style={{ height: 'clamp(100px, 15vw, 120px)' }}>
                  {chartsReady && allocChart.size.width > 0 && allocChart.size.height > 0 && (
                    <PieChart width={allocChart.size.width} height={allocChart.size.height}>
                      <Pie
                        data={allocData}
                        cx="50%"
                        cy="50%"
                        innerRadius={36}
                        outerRadius={52}
                        dataKey="value"
                        stroke="none"
                        isAnimationActive={true}
                        animationDuration={700}
                      >
                        {allocData.map((entry, i) => (
                          <Cell key={i} fill={entry.color} opacity={0.85} />
                        ))}
                      </Pie>
                    </PieChart>
                  )}
                </div>
              </HeroCard>

              {/* Zero-Knowledge */}
              <HeroCard delay={220}>
                <div className="flex flex-col gap-1">
                  <span style={{ fontSize: 'clamp(13px, 2vw, 14px)', fontWeight: 600, color: 'var(--text-primary)' }}>
                    Zero-Knowledge
                  </span>
                  <span className="flex items-center gap-1.5" style={{ fontSize: 'clamp(11px, 2vw, 12px)', color: 'var(--text-tertiary)' }}>
                    <Lock size={12} />
                    Encrypted Vault
                  </span>
                </div>
              </HeroCard>
            </div>
          </div>
        </div>
      </section>

      {/* Features Section */}
      <section
        id="produit"
        className="px-4 sm:px-6"
        style={{
          scrollMarginTop: '112px',
          paddingTop: 'clamp(64px, 10vw, 96px)',
          paddingBottom: 'clamp(64px, 10vw, 96px)',
        }}
      >
        <div className="max-w-[1440px] mx-auto">
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 sm:gap-6">
            <Card level={1}>
              <Layers
                size={32}
                style={{
                  color: 'var(--color-accent)',
                  marginBottom: 'clamp(12px, 2vw, 16px)',
                }}
              />
              <h3
                className="mb-2"
                style={{
                  fontSize: 'clamp(16px, 3vw, 18px)',
                  fontWeight: 600,
                  color: 'var(--text-primary)',
                }}
              >
                Multi-portefeuille
              </h3>
              <p
                style={{
                  fontSize: 'clamp(14px, 2.5vw, 16px)',
                  lineHeight: '1.6',
                  color: 'var(--text-secondary)',
                }}
              >
                Regroupez vos actions, cryptos et ETF dans un espace unifié.
              </p>
            </Card>

            <Card level={1}>
              <Calculator
                size={32}
                style={{
                  color: 'var(--color-accent)',
                  marginBottom: 'clamp(12px, 2vw, 16px)',
                }}
              />
              <h3
                className="mb-2"
                style={{
                  fontSize: 'clamp(16px, 3vw, 18px)',
                  fontWeight: 600,
                  color: 'var(--text-primary)',
                }}
              >
                Calculs au centime
              </h3>
              <p
                style={{
                  fontSize: 'clamp(14px, 2.5vw, 16px)',
                  lineHeight: '1.6',
                  color: 'var(--text-secondary)',
                }}
              >
                Architecture transactionnelle. Chaque calcul est traçable et vérifiable.
              </p>
            </Card>

            <Card level={1}>
              <Shield
                size={32}
                style={{
                  color: 'var(--color-accent)',
                  marginBottom: 'clamp(12px, 2vw, 16px)',
                }}
              />
              <h3
                className="mb-2"
                style={{
                  fontSize: 'clamp(16px, 3vw, 18px)',
                  fontWeight: 600,
                  color: 'var(--text-primary)',
                }}
              >
                Privacy radicale
              </h3>
              <p
                style={{
                  fontSize: 'clamp(14px, 2.5vw, 16px)',
                  lineHeight: '1.6',
                  color: 'var(--text-secondary)',
                }}
              >
                Pas d'email. Pas d'identité. Vos données restent vôtres.
              </p>
            </Card>
          </div>
        </div>
      </section>

      {/* Security Section */}
      <section
        id="securite"
        className="px-4 sm:px-6"
        style={{
          scrollMarginTop: '112px',
          paddingTop: 'clamp(64px, 10vw, 96px)',
          paddingBottom: 'clamp(64px, 10vw, 96px)',
        }}
      >
        <div className="max-w-[1100px] mx-auto">
          <h2
            className="text-center px-4"
            style={{
              fontSize: 'clamp(24px, 5vw, 30px)',
              fontWeight: 800,
              color: 'var(--text-primary)',
            }}
          >
            Sécurité radicale
          </h2>
          <p
            className="text-center px-4"
            style={{
              fontSize: 'clamp(14px, 2.5vw, 16px)',
              color: 'var(--text-secondary)',
              marginTop: 'clamp(8px, 1.5vw, 12px)',
            }}
          >
            Aucune donnée personnelle. Aucun compromis.
          </p>

          <div
            className="grid grid-cols-1 md:grid-cols-2"
            style={{
              marginTop: 'clamp(32px, 6vw, 48px)',
              gap: 'clamp(24px, 5vw, 48px)',
            }}
          >
            {/* Left — Bullet list */}
            <div
              className="flex flex-col px-4"
              style={{ gap: 'clamp(12px, 2vw, 16px)' }}
            >
              {[
                'Aucun email requis',
                'Mot de passe + code de récupération',
                'Phrases de récupération personnelles',
                'Base de données non exploitable',
              ].map((item) => (
                <div
                  key={item}
                  className="flex items-start sm:items-center"
                  style={{ gap: 'clamp(10px, 2vw, 12px)', minHeight: '44px' }}
                >
                  <Check
                    size={18}
                    style={{
                      color: 'var(--color-accent)',
                      flexShrink: 0,
                      marginTop: '2px',
                    }}
                  />
                  <span
                    style={{
                      fontSize: 'clamp(14px, 2.5vw, 16px)',
                      color: 'var(--text-primary)',
                    }}
                  >
                    {item}
                  </span>
                </div>
              ))}
            </div>

            {/* Right — Reassurance card */}
            <Card level={1}>
              <ShieldCheck
                size={24}
                style={{
                  color: 'var(--color-accent)',
                  marginBottom: 'clamp(10px, 2vw, 12px)',
                }}
              />
              <h3
                style={{
                  fontSize: 'clamp(15px, 2.5vw, 16px)',
                  fontWeight: 600,
                  color: 'var(--text-primary)',
                }}
              >
                Conçu pour la confidentialité
              </h3>
              <p
                style={{
                  fontSize: 'clamp(13px, 2.2vw, 14px)',
                  color: 'var(--text-secondary)',
                  marginTop: 'clamp(6px, 1.5vw, 8px)',
                  lineHeight: '1.6',
                }}
              >
                Kushim ne collecte ni email, ni identité. En cas de compromission de nos serveurs,
                vos données personnelles restent protégées par conception.
              </p>
            </Card>
          </div>
        </div>
      </section>

      {/* Pricing Section */}
      <section
        id="tarifs"
        className="px-4 sm:px-6"
        style={{
          scrollMarginTop: '112px',
          paddingTop: 'clamp(64px, 10vw, 96px)',
          paddingBottom: 'clamp(64px, 10vw, 96px)',
        }}
      >
        <div className="max-w-[90vw] sm:max-w-[420px] mx-auto">
          <Card level={1}>
            <div className="text-center" style={{ marginBottom: 'clamp(20px, 3vw, 24px)' }}>
              <div
                className="flex items-baseline justify-center gap-2 mb-2"
                style={{ flexWrap: 'wrap' }}
              >
                <span
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: 'clamp(36px, 8vw, 48px)',
                    fontWeight: 800,
                    color: 'var(--text-primary)',
                  }}
                >
                  €3.49
                </span>
                <span
                  style={{
                    fontSize: 'clamp(13px, 2.5vw, 14px)',
                    color: 'var(--text-secondary)',
                  }}
                >
                  /mois
                </span>
              </div>
            </div>

            <div
              className="mb-6"
              style={{ display: 'flex', flexDirection: 'column', gap: 'clamp(10px, 2vw, 12px)' }}
            >
              {[
                'Lorem ipsum dolor sit amet',
                'Consectetur adipiscing elit',
                'Sed do eiusmod tempor incididunt',
                'Ut enim ad minim veniam',
              ].map((feature) => (
                <div
                  key={feature}
                  className="flex items-start sm:items-center"
                  style={{ gap: 'clamp(8px, 1.5vw, 10px)', minHeight: '44px' }}
                >
                  <Check
                    size={16}
                    style={{
                      color: 'var(--color-gain)',
                      flexShrink: 0,
                      marginTop: '2px',
                    }}
                  />
                  <span
                    style={{
                      fontSize: 'clamp(13px, 2.5vw, 14px)',
                      color: 'var(--text-primary)',
                    }}
                  >
                    {feature}
                  </span>
                </div>
              ))}
            </div>

            <Link to={`${authUrl}/inscription`} className="block w-full">
              <Button variant="primary" className="w-full min-h-[44px]">
                Commencer
              </Button>
            </Link>
          </Card>
        </div>
      </section>
    </div>
  );
}
