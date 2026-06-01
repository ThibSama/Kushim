import React from 'react';
import { TrendingUp, TrendingDown } from 'lucide-react';

interface KPICardProps {
  label: string;
  value: string;
  change?: {
    value: string;
    isPositive: boolean;
  };
  className?: string;
}

export function KPICard({ label, value, change, className = '' }: KPICardProps) {
  return (
    <div
      className={`glass glass-hover ${className}`}
      style={{
        borderRadius: 'var(--radius-xl)',
        padding: '24px',
      }}
    >
      <div
        className="uppercase mb-2"
        style={{
          fontSize: '12px',
          color: 'var(--text-tertiary)',
          letterSpacing: '0.05em',
        }}
      >
        {label}
      </div>
      <div
        className="mb-2"
        style={{
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: '32px',
          fontWeight: 700,
          fontVariantNumeric: 'tabular-nums',
          color: 'var(--text-primary)',
        }}
      >
        {value}
      </div>
      {change && (
        <div className="flex items-center gap-1">
          {change.isPositive ? (
            <TrendingUp size={14} style={{ color: 'var(--color-gain)' }} />
          ) : (
            <TrendingDown size={14} style={{ color: 'var(--color-loss)' }} />
          )}
          <span
            style={{
              fontSize: '12px',
              fontWeight: 600,
              color: change.isPositive ? 'var(--color-gain)' : 'var(--color-loss)',
            }}
          >
            {change.value}
          </span>
        </div>
      )}
    </div>
  );
}
