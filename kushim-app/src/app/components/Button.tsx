import React, { useState } from 'react';
import { LucideIcon } from 'lucide-react';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  icon?: LucideIcon;
  children: React.ReactNode;
}

export function Button({
  variant = 'primary',
  icon: Icon,
  children,
  className = '',
  disabled,
  ...props
}: ButtonProps) {
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);

  const baseStyles = 'rounded-[9999px] flex items-center justify-center transition-all glass-interactive';

  const scale = disabled ? 1 : pressed ? 0.98 : hovered ? 1.02 : 1;

  const variantColors: Record<string, React.CSSProperties> = {
    primary: {
      background: hovered && !disabled
        ? 'linear-gradient(180deg, rgba(255,255,255,0.12), rgba(255,255,255,0.04)), var(--cta-hover-bg)'
        : 'linear-gradient(180deg, rgba(255,255,255,0.16), rgba(255,255,255,0.04)), var(--color-cta-bg)',
      color: 'var(--color-cta-text)',
      boxShadow: hovered && !disabled
        ? 'inset 0 1px 0 rgba(255,255,255,0.18), 0 18px 36px rgba(0, 0, 0, 0.24)'
        : 'inset 0 1px 0 rgba(255,255,255,0.16), 0 12px 28px rgba(0, 0, 0, 0.18)',
    },
    secondary: {
      background:
        hovered && !disabled
          ? 'linear-gradient(180deg, rgba(255,255,255,0.08), rgba(255,255,255,0.03)), var(--surface-2-bg)'
          : 'linear-gradient(180deg, rgba(255,255,255,0.05), rgba(255,255,255,0.02)), transparent',
      color: 'var(--text-secondary)',
      borderColor: hovered ? 'var(--text-tertiary)' : 'var(--surface-1-border)',
      backdropFilter: 'var(--glass-blur)',
      WebkitBackdropFilter: 'var(--glass-blur)',
      boxShadow: 'var(--glass-highlight)',
    },
    ghost: {
      background:
        hovered && !disabled
          ? 'linear-gradient(180deg, rgba(255,255,255,0.06), rgba(255,255,255,0.02)), var(--ghost-hover-bg)'
          : 'transparent',
      color: hovered && !disabled ? 'var(--text-primary)' : 'var(--ghost-text)',
      borderColor: hovered && !disabled ? 'var(--glass-border)' : 'transparent',
    },
    danger: {
      background:
        hovered && !disabled
          ? 'linear-gradient(180deg, rgba(239,68,68,0.10), rgba(239,68,68,0.03)), rgba(239, 68, 68, 0.06)'
          : 'linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.01)), transparent',
      color: 'var(--color-loss)',
      borderColor: hovered ? 'var(--color-loss)' : 'var(--surface-1-border)',
      backdropFilter: 'var(--glass-blur)',
      WebkitBackdropFilter: 'var(--glass-blur)',
      boxShadow: 'var(--glass-highlight)',
    },
  };

  const borderVariants = ['secondary', 'danger'];

  return (
    <button
      className={`${baseStyles} ${className}`}
      disabled={disabled}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        setHovered(false);
        setPressed(false);
      }}
      onMouseDown={() => setPressed(true)}
      onMouseUp={() => setPressed(false)}
      style={{
        fontSize: 'clamp(14px, 2.5vw, 15px)',
        fontWeight: 500,
        minHeight: '44px',
        padding: 'clamp(10px, 2vw, 12px) clamp(24px, 4vw, 28px)',
        gap: 'clamp(6px, 1.5vw, 8px)',
        border: borderVariants.includes(variant)
          ? `1px solid ${variantColors[variant].borderColor}`
          : variant === 'ghost'
            ? `1px solid ${variantColors[variant].borderColor ?? 'transparent'}`
            : 'none',
        ...variantColors[variant],
        transform: disabled ? `scale(${scale})` : `translateY(${hovered ? '-1px' : '0px'}) scale(${scale})`,
        transition: pressed
          ? 'transform 80ms ease, background 150ms ease'
          : 'transform 150ms ease, background 150ms ease',
        opacity: disabled ? 0.5 : 1,
        cursor: disabled ? 'not-allowed' : 'pointer',
        pointerEvents: disabled ? 'auto' : undefined,
      }}
      {...props}
    >
      {Icon && <Icon size={16} />}
      {children}
    </button>
  );
}
