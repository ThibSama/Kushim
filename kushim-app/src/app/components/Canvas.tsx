import React from 'react';

export function Canvas() {
  return (
    <div className="fixed inset-0 pointer-events-none z-0">
      <svg
        className="w-full h-full"
        xmlns="http://www.w3.org/2000/svg"
        style={{ opacity: 1 }}
      >
        <defs>
          <pattern
            id="canvas-pattern"
            x="0"
            y="0"
            width="100"
            height="100"
            patternUnits="userSpaceOnUse"
          >
            {/* Algorithmic curves - subtle chaotic attractors inspired pattern */}
            <path
              d="M 20 20 Q 40 10, 60 30 T 100 40"
              stroke="var(--canvas-lines)"
              strokeWidth="0.5"
              fill="none"
            />
            <path
              d="M 0 50 Q 30 40, 50 60 T 90 70"
              stroke="var(--canvas-lines)"
              strokeWidth="0.5"
              fill="none"
            />
            <path
              d="M 40 0 Q 50 30, 70 20 T 80 80"
              stroke="var(--canvas-lines)"
              strokeWidth="0.5"
              fill="none"
            />
            <path
              d="M 10 80 Q 30 70, 40 90 T 80 85"
              stroke="var(--canvas-lines)"
              strokeWidth="0.5"
              fill="none"
            />
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#canvas-pattern)" />
      </svg>
    </div>
  );
}
