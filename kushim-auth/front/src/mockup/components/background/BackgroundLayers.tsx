"use client";

import React from "react";
import { Starfield } from "./Starfield";
import { Particles } from "./Particles";
import { DotGrid } from "./DotGrid";

export function BackgroundLayers() {
  return (
    <>
      <Starfield />
      <Particles />
      <DotGrid />
    </>
  );
}
