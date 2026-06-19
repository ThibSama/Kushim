"use client";

import React, { useEffect, useRef } from "react";

type Particle = {
  homeX: number;
  homeY: number;
  x: number;
  y: number;
  velocityX: number;
  velocityY: number;
  radius: number;
  phase: number;
};

type MouseState = {
  x: number;
  y: number;
  active: boolean;
};

const PARTICLE_DENSITY_DIVISOR = 25000;
const PARTICLE_MAX_COUNT = 120;
const INTERACTION_RADIUS = 120;

function randomBetween(min: number, max: number): number {
  return Math.random() * (max - min) + min;
}

function buildParticles(width: number, height: number): Particle[] {
  const count = Math.min(
    PARTICLE_MAX_COUNT,
    Math.floor((width * height) / PARTICLE_DENSITY_DIVISOR),
  );
  const particles: Particle[] = [];

  for (let i = 0; i < count; i += 1) {
    const homeX = Math.random() * width;
    const homeY = Math.random() * height;
    particles.push({
      homeX,
      homeY,
      x: homeX,
      y: homeY,
      velocityX: randomBetween(-0.15, 0.15),
      velocityY: randomBetween(-0.15, 0.15),
      radius: randomBetween(1, 2.5),
      phase: randomBetween(0, Math.PI * 2),
    });
  }

  return particles;
}

export function Particles() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const particlesRef = useRef<Particle[]>([]);
  const viewportRef = useRef({ width: 0, height: 0, dpr: 1 });
  const mouseRef = useRef<MouseState>({ x: 0, y: 0, active: false });

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
      particlesRef.current = buildParticles(width, height);
    };

    const handleMouseMove = (event: MouseEvent) => {
      mouseRef.current = {
        x: event.clientX,
        y: event.clientY,
        active: true,
      };
    };

    const handleMouseLeave = () => {
      mouseRef.current.active = false;
    };

    const drawFrame = (time: number) => {
      const { width, height } = viewportRef.current;
      const mouse = mouseRef.current;
      context.clearRect(0, 0, width, height);

      for (const particle of particlesRef.current) {
        const orbitalX = Math.sin(time * 0.00022 + particle.phase) * 6;
        const orbitalY = Math.cos(time * 0.00018 + particle.phase) * 6;
        particle.homeX += particle.velocityX;
        particle.homeY += particle.velocityY;

        if (particle.homeX < 0 || particle.homeX > width) {
          particle.velocityX *= -1;
          particle.homeX = Math.max(0, Math.min(width, particle.homeX));
        }
        if (particle.homeY < 0 || particle.homeY > height) {
          particle.velocityY *= -1;
          particle.homeY = Math.max(0, Math.min(height, particle.homeY));
        }

        const targetX = particle.homeX + orbitalX;
        const targetY = particle.homeY + orbitalY;

        particle.x += (targetX - particle.x) * 0.06;
        particle.y += (targetY - particle.y) * 0.06;

        if (mouse.active) {
          const dx = particle.x - mouse.x;
          const dy = particle.y - mouse.y;
          const distance = Math.hypot(dx, dy);

          if (distance > 0 && distance < INTERACTION_RADIUS) {
            const normalized = 1 - distance / INTERACTION_RADIUS;
            const force = normalized * normalized * 2.4;
            particle.x += (dx / distance) * force;
            particle.y += (dy / distance) * force;
          }
        }

        context.beginPath();
        context.fillStyle = "rgba(180, 180, 220, 0.35)";
        context.arc(particle.x, particle.y, particle.radius, 0, Math.PI * 2);
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
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseout", handleMouseLeave);

    return () => {
      if (animationFrameRef.current !== null) {
        window.cancelAnimationFrame(animationFrameRef.current);
      }
      window.removeEventListener("resize", handleResize);
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseout", handleMouseLeave);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="background-layer background-particles"
      aria-hidden="true"
    />
  );
}
