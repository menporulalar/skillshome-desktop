// Renders the source app icon (src-tauri/icons/icon.png) from the shared brand
// asset. `npm run brand:icon` runs this and then `tauri icon`, which fans the
// source out to every platform size AND rewrites icon.png itself at its own
// default resolution -- so run the two together, never this script alone.
//
// Keep this the ONLY step that colours the icon set so the brass mark can't
// drift back to a stale brand again (last drift: regenerated 2026-07-27 in the
// old purple brand, then missed by both the blue and the brass token bumps --
// `src-tauri/icons/` is not covered by `npm run brand:check`).
//
// The brand SVG (brand/logo/assets/skillshome-icon.svg) is the warm-black
// (#100d0b) rounded tile + three brass chevrons brightening downward. It bakes
// the rounded corners, matching how the icon set was already composed (rounded
// tile on transparency, not a full-bleed square); `tauri icon` then applies its
// own per-platform masking on top.

import { readFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const SRC_SVG = resolve(repoRoot, 'brand/logo/assets/skillshome-icon.svg');
const OUT_PNG = resolve(repoRoot, 'src-tauri/icons/icon.png');
const SIZE = 1024; // tauri icon wants a >=1024 square source; it downsizes icon.png afterwards

const svg = readFileSync(SRC_SVG);
mkdirSync(dirname(OUT_PNG), { recursive: true });

await sharp(svg, { density: 384 })
  .resize(SIZE, SIZE, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } })
  .png()
  .toFile(OUT_PNG);

console.log(`Rendered ${OUT_PNG} at ${SIZE}x${SIZE} from ${SRC_SVG}.`);
console.log('Now run `tauri icon src-tauri/icons/icon.png` (or use `npm run brand:icon`).');
