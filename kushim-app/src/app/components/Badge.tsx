import React from 'react';

interface BadgeProps {
  children: React.ReactNode;
  variant?: 'gain' | 'loss' | 'neutral' | 'warning' | 'info';
  className?: string;
}

export function Badge({ children, variant = 'neutral', className = '' }: BadgeProps) {
  const variantStyles = {
    gain: {
      background: 'rgba(16, 185, 129, 0.12)',
      color: 'var(--color-gain)',
    },
    loss: {
      background: 'rgba(239, 68, 68, 0.12)',
      color: 'var(--color-loss)',
    },
    neutral: {
      background: 'rgba(161, 161, 170, 0.12)',
      color: 'var(--color-neutral)',
    },
    warning: {
      background: 'rgba(245, 158, 11, 0.12)',
      color: 'var(--color-warning)',
    },
    info: {
      background: 'rgba(59, 130, 246, 0.12)',
      color: 'var(--color-accent)',
    },
  };
  
  return (
    <span
      className={`inline-flex items-center rounded-[9999px] ${className}`}
      style={{
        fontSize: 'clamp(11px, 2vw, 12px)',
        fontWeight: 600,
        letterSpacing: '0.02em',
        gap: 'clamp(4px, 1vw, 6px)',
        padding: 'clamp(4px, 1vw, 5px) clamp(10px, 2vw, 12px)',
        ...variantStyles[variant],
      }}
    >
      {children}
    </span>
  );
}
