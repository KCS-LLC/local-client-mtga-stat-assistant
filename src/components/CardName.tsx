import { useRef, useState } from "react";
import { createPortal } from "react-dom";

export type CardImageSource = "scryfall" | "gatherer";

interface CachedUrls {
  scryfall: string | null;
  gatherer: string | null;
}

// Module-level cache so each card name is only fetched once per session.
const urlCache = new Map<string, CachedUrls | "error">();
// In-flight promises so concurrent hovers on the same name share one fetch.
const inflight = new Map<string, Promise<CachedUrls | null>>();

async function resolveUrls(name: string): Promise<CachedUrls | null> {
  const hit = urlCache.get(name);
  if (hit !== undefined) return hit === "error" ? null : hit;
  if (inflight.has(name)) return inflight.get(name)!;

  const promise = (async (): Promise<CachedUrls | null> => {
    try {
      let resp = await fetch(
        `https://api.scryfall.com/cards/named?exact=${encodeURIComponent(name)}`,
      );
      // Rebalanced MTGA cards have an "A-" prefix not recognised by Scryfall.
      if (!resp.ok && name.startsWith("A-")) {
        resp = await fetch(
          `https://api.scryfall.com/cards/named?exact=${encodeURIComponent(name.slice(2))}`,
        );
      }
      if (!resp.ok) {
        urlCache.set(name, "error");
        return null;
      }
      const data = await resp.json();
      // Top-level image_uris covers single-faced cards and DFC front faces.
      const scryfallUrl: string | null = data.image_uris?.normal ?? null;
      const multiverseId: number | null = data.multiverse_ids?.[0] ?? null;
      const gathererUrl: string | null = multiverseId
        ? `https://gatherer.wizards.com/Handlers/Image.ashx?multiverseid=${multiverseId}&type=card`
        : null;
      const urls: CachedUrls = { scryfall: scryfallUrl, gatherer: gathererUrl };
      urlCache.set(name, urls);
      return urls;
    } catch {
      urlCache.set(name, "error");
      return null;
    } finally {
      inflight.delete(name);
    }
  })();

  inflight.set(name, promise);
  return promise;
}

function getImageUrl(urls: CachedUrls, source: CardImageSource): string | null {
  if (source === "gatherer") {
    return urls.gatherer ?? urls.scryfall; // fall back to Scryfall for Arena-only cards
  }
  return urls.scryfall;
}

interface Props {
  name: string;
  className?: string;
  /** Text rendered before the name (e.g. "↑ " for known top-of-library). */
  prefix?: string;
}

const IMG_W = 223;
const IMG_H = 310;

export function CardName({ name, className, prefix }: Props) {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [urls, setUrls] = useState<CachedUrls | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  function handleMouseEnter(e: React.MouseEvent) {
    setPos({ x: e.clientX, y: e.clientY });
    timer.current = setTimeout(() => {
      resolveUrls(name).then((u) => { if (u) setUrls(u); });
    }, 150);
  }

  function handleMouseMove(e: React.MouseEvent) {
    if (pos) setPos({ x: e.clientX, y: e.clientY });
  }

  function handleMouseLeave() {
    if (timer.current) clearTimeout(timer.current);
    setPos(null);
  }

  const source = (localStorage.getItem("cardImageSource") as CardImageSource | null) ?? "scryfall";
  const imageUrl = urls ? getImageUrl(urls, source) : null;

  // Keep the popup inside the viewport.
  const left = pos ? Math.min(pos.x + 16, window.innerWidth - IMG_W - 8) : 0;
  const top = pos
    ? Math.max(8, Math.min(pos.y - Math.round(IMG_H / 2), window.innerHeight - IMG_H - 8))
    : 0;

  return (
    <>
      <span
        className={className}
        onMouseEnter={handleMouseEnter}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      >
        {prefix}{name}
      </span>
      {pos &&
        imageUrl &&
        createPortal(
          <img
            src={imageUrl}
            alt={name}
            width={IMG_W}
            height={IMG_H}
            style={{ left, top }}
            className="fixed pointer-events-none rounded-lg shadow-2xl z-50"
          />,
          document.body,
        )}
    </>
  );
}
