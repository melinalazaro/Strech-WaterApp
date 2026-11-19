import assert from "node:assert/strict";
import test from "node:test";
import { prepareArtwork } from "../src/media.ts";

const art = {
  label: "Agua",
  sequence: { src: "water", frames: 32, cycleMs: 3000 },
  reserve: { src: "placeholder", frames: 1, cycleMs: 0 },
};

test("a broken sequence decodes the reserve before acknowledging readiness", async () => {
  const order = [];
  const result = await prepareArtwork(art, async candidate => {
    order.push(candidate.src);
    if (candidate === art.sequence) throw Error("decode failed");
  });
  assert.deepEqual(order, ["water", "placeholder"]);
  assert.equal(result, art.reserve);
});

test("failure of both images propagates to the native preparation failure path", async () => {
  await assert.rejects(prepareArtwork(art, async () => { throw Error("decode failed"); }), /decode failed/);
});
