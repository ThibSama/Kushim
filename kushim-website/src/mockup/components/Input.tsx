"use client";

import React from 'react';

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helperText?: string;
}

export function Input({ label, error, helperText, className = '', ...props }: InputProps) {
  return (
    <div className="w-full">
      {label && (
        <label
          className="block mb-1.5"
          style={{
            fontSize: '12px',
            fontWeight: 500,
            color: 'var(--text-secondary)',
          }}
        >
          {label}
        </label>
      )}
      <input
        className={`glass-field w-full px-5 py-3 rounded-[9999px] transition-all ${className}`}
        style={{
          border: error ? '1px solid var(--color-loss)' : '1px solid var(--surface-2-border)',
          fontSize: '15px',
          color: 'var(--text-primary)',
          transition: 'all var(--transition-base)',
        }}
        {...props}
      />
      {error && (
        <p className="mt-1" style={{ fontSize: '12px', color: 'var(--color-loss)' }}>
          {error}
        </p>
      )}
      {helperText && !error && (
        <p className="mt-1" style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
          {helperText}
        </p>
      )}
    </div>
  );
}
