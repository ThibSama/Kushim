"use client";

import React, { useEffect, useRef } from "react";

type Star = {
  x: number;
  y: number;
  radius: number;
  opacity: number;
  speed: number;
  phase: number;
  blueTint: boolean;
};

const STAR_DENSITY_DIVISOR = 8000;
const STAR_MAX_COUNT = 400;

function randomBetween(min: number, max: number): number {
  return Math.random() * (max - min) + min;
}

function buildStars(width: number, height: number): Star[] {
  const count = Math.min(
    STAR_MAX_COUNT,
    Math.floor((width * height) / STAR_DENSITY_DIVISOR),
  );
  const stars: Star[] = [];

  for (let i = 0; i < count; i += 1) {
    stars.push({
      x: Math.random() * width,
      y: Math.random() * height,
      radius: randomBetween(0.4, 1.6),
      opacity: randomBetween(0.2, 0.9),
      speed: randomBetween(0.05, 0.25),
      phase: randomBetween(0, Math.PI * 2),
      blueTint: Math.random() < 0.15,
    });
  }

  return stars;
}

export function Starfield() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const starsRef = useRef<Star[]>([]);
  const viewportRef = useRef({ width: 0, height: 0, dpr: 1 });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const handleResize = () => {
      const width = window.innerWidth;
      const height = window.innerHeight;
      const dpr = Math.max(1, window.devicePixelRatio || 1);

      viewportRef.current = { width, height, dpr };

      canvas.width = Math.floor(width * dpr);
      canvas.height = Math.floor(height * dpr);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;

      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      starsRef.current = buildStars(width, height);
    };

    const drawFrame = (time: number) => {
      const { width, height } = viewportRef.current;
      context.clearRect(0, 0, width, height);

      for (const star of starsRef.current) {
        star.y += star.speed;
        if (star.y - star.radius > height) {
          star.y = -star.radius;
          star.x = Math.random() * width;
        }

        const twinkle = Math.sin(time * 0.0007 + star.phase) * 0.1;
        const alpha = Math.max(0.05, Math.min(1, star.opacity + twinkle));
        const color = star.blueTint
          ? `rgba(200, 220, 255, ${alpha})`
          : `rgba(255, 255, 255, ${alpha})`;

        context.beginPath();
        context.fillStyle = color;
        context.arc(star.x, star.y, star.radius, 0, Math.PI * 2);
        context.fill();
      }

      if (!reducedMotion) {
        animationFrameRef.current = window.requestAnimationFrame(drawFrame);
      }
    };

    handleResize();
    if (reducedMotion) {
      drawFrame(0);
    } else {
      animationFrameRef.current = window.requestAnimationFrame(drawFrame);
    }
    window.addEventListener("resize", handleResize);

    return () => {
      if (animationFrameRef.current !== null) {
        window.cancelAnimationFrame(animationFrameRef.current);
      }
      window.removeEventListener("resize", handleResize);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="background-layer background-starfield"
      aria-hidden="true"
    />
  );
}
