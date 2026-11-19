import stretch from "./assets/stretch-poses.svg";
import water from "./assets/water-poses.svg";
import placeholder from "./assets/placeholder.svg";
import type { Artwork } from "./media";

const reserve = { src: placeholder, frames: 1, cycleMs: 0 };
export const catalog = {
  stretch: { label: "Estiramiento", sequence: { src: stretch, frames: 32, cycleMs: 3200 }, reserve },
  water: { label: "Agua", sequence: { src: water, frames: 32, cycleMs: 3000 }, reserve },
} satisfies Record<string, Artwork>;
