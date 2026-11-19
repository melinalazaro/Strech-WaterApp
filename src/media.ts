/** An SVG strip has one row of square poses. A single pose is a static reserve. */
export type Sequence = { src: string; frames: number; cycleMs: number };
export type Artwork = { label: string; sequence: Sequence; reserve: Sequence };

export async function prepareArtwork(
  art: Artwork,
  decode: (sequence: Sequence) => Promise<void>,
): Promise<Sequence> {
  try {
    await decode(art.sequence);
    return art.sequence;
  } catch {
    await decode(art.reserve);
    return art.reserve;
  }
}
