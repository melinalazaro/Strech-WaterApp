import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { catalog } from "./catalog";
import { prepareArtwork, type Sequence } from "./media";
import "./styles.css";

type Request = { id: number; kind: keyof typeof catalog; test: boolean };
type Start = { id: number; durationMs: number };
declare global {
  interface Window {
    __REMINDER__?: Request;
    __TAURI__?: { core: { invoke: (command: string, args: Record<string, unknown>) => Promise<unknown> } };
  }
}

function Appearance({ request }: { request: Request }) {
  const scene = useRef<HTMLElement>(null);
  const strip = useRef<HTMLImageElement>(null);
  const art = catalog[request.kind];

  useEffect(() => {
    const image = strip.current!;
    const root = scene.current!;
    let current = true;
    let started = false;
    let sequence: Sequence | undefined;
    const animations: Animation[] = [];

    function end() {
      current = false;
      animations.splice(0).forEach(animation => animation.cancel());
      root.classList.remove("visible");
    }
    function begin(event: Event) {
      const { id, durationMs } = (event as CustomEvent<Start>).detail;
      if (!current || started || !sequence || id !== request.id || durationMs <= 0) return;
      started = true;
      root.classList.add("visible");
      const fade = Math.min(250 / durationMs, 0.5);
      animations.push(root.animate([
        { opacity: 0, offset: 0 }, { opacity: 1, offset: fade },
        { opacity: 1, offset: 1 - fade }, { opacity: 0, offset: 1 },
      ], { duration: durationMs, fill: "both" }));
      if (sequence.frames > 1) {
        animations.push(image.animate(
          [{ transform: "translateX(0)" }, { transform: "translateX(-100%)" }],
          { duration: sequence.cycleMs, iterations: Infinity, easing: `steps(${sequence.frames}, end)` },
        ));
      }
    }
    function acknowledge(ok: boolean, fallback: boolean) {
      if (current) void window.__TAURI__?.core.invoke("presentation_ready", { id: request.id, ok, fallback })
        .catch(() => { /* Rust also has a preparation timeout. */ });
    }
    window.addEventListener("reminder-start", begin);
    window.addEventListener("reminder-hidden", end);
    void prepareArtwork(art, async candidate => {
      image.src = candidate.src;
      await image.decode();
      if (!image.naturalHeight || image.naturalWidth !== candidate.frames * image.naturalHeight) {
        throw new Error("Invalid pose strip");
      }
    }).then(ready => {
      sequence = ready;
      acknowledge(true, ready === art.reserve);
    }, () => acknowledge(false, true));

    return () => {
      end();
      window.removeEventListener("reminder-start", begin);
      window.removeEventListener("reminder-hidden", end);
    };
  }, [request.id, art]);

  return <main ref={scene} aria-label={art.label}>
    <div className="pose"><img ref={strip} alt="" draggable={false} /></div>
    <div className="label">{request.test ? "PRUEBA · " : ""}{art.label}</div>
  </main>;
}
function Reminder() {
  const [request, setRequest] = useState(window.__REMINDER__);
  useEffect(() => {
    const receive = () => setRequest(window.__REMINDER__);
    window.addEventListener("reminder", receive);
    receive();
    return () => window.removeEventListener("reminder", receive);
  }, []);
  return request ? <Appearance key={request.id} request={request} /> : null;
}
createRoot(document.getElementById("root")!).render(<Reminder />);
