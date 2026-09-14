// Renders the aicross app mark — an "AC" monogram, light glyph on a dark
// rounded tile — to `icons/icon-master.png` at 1024x1024, plus the two sizes
// `tauri icon` does not emit but `tauri.conf.json` references (256, 512).
//
// The rest of the icon set comes from that master via the official tool:
//
//     node scripts/generate-aicross-icons.mjs
//     npx --yes -p @tauri-apps/cli@2 tauri icon icons/icon-master.png -o icons
//
// Pure Node (no image deps): an SDF rasteriser with 4x4 supersampling plus a
// hand-rolled zlib/PNG writer. The previous revision drew the letters from a
// 5x7 bitmap font, which stair-stepped badly above 64px.
//
// The same geometry also lives as vectors in icons/icon-master.svg and
// src-ui/public/favicon.svg. Change one, change all three.

import { writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const SIZE = 1024;   // master canvas
const SS = 4;        // supersample samples per axis

// Dark ground (deliberately not pure black), off-white glyph. No accent.
const BG = [0x14, 0x17, 0x1c];
const FG = [0xf1, 0xf3, 0xf6];

const TILE_R = 224;                 // rounded-square corner radius (~22%)
const STROKE = 76, R = STROKE / 2;  // main stroke weight

const yTop = 342, yBot = 682;       // cap height 340, optically centred
const aL = 168, aR = 468, aApex = 318;
// The C overshoots cap height by ~4%, the usual optical correction for a round.
const cx = 686, cy = 512, CR = (354 - STROKE) / 2;

// The A crossbar sits low and runs thinner than the legs, so the counter above
// it stays open when the icon is scaled down to 32px.
const BAR_R = 32;
const barY = yTop + 0.68 * (yBot - yTop);
const tBar = (barY - yTop) / (yBot - yTop);
const barL = aApex - (aApex - aL) * tBar + 12;
const barR = aApex + (aR - aApex) * tBar - 12;

// The C's aperture faces right: the arc runs 40deg -> 320deg through 180deg.
const A0 = (40 * Math.PI) / 180;
const A1 = (320 * Math.PI) / 180;

const clamp = (v, a, b) => (v < a ? a : v > b ? b : v);

function sdSegment(px, py, ax, ay, bx, by) {
  const pax = px - ax, pay = py - ay, bax = bx - ax, bay = by - ay;
  const h = clamp((pax * bax + pay * bay) / (bax * bax + bay * bay), 0, 1);
  return Math.hypot(pax - bax * h, pay - bay * h);
}

function sdRoundBox(px, py, hx, hy, r) {
  const qx = Math.abs(px) - hx + r, qy = Math.abs(py) - hy + r;
  return (
    Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) +
    Math.min(Math.max(qx, qy), 0) -
    r
  );
}

/** Distance to the C's centreline: the arc band plus a round cap at each end. */
function sdArc(px, py) {
  const dx = px - cx, dy = py - cy;
  let ang = Math.atan2(-dy, dx);
  if (ang < 0) ang += 2 * Math.PI;
  if (ang >= A0 && ang <= A1) return Math.abs(Math.hypot(dx, dy) - CR);
  const e0x = cx + CR * Math.cos(A0), e0y = cy - CR * Math.sin(A0);
  const e1x = cx + CR * Math.cos(A1), e1y = cy - CR * Math.sin(A1);
  return Math.min(Math.hypot(px - e0x, py - e0y), Math.hypot(px - e1x, py - e1y));
}

/** <= 0 means the point is inside the glyph. */
function glyphDist(px, py) {
  const bar = sdSegment(px, py, barL, barY, barR, barY) - BAR_R;
  const rest =
    Math.min(
      sdSegment(px, py, aApex, yTop, aL, yBot),
      sdSegment(px, py, aApex, yTop, aR, yBot),
      sdArc(px, py),
    ) - R;
  return Math.min(bar, rest);
}

/** Rasterises the mark at `size` px; all geometry above is in master units. */
function render(size) {
  const stride = size * 4 + 1;
  const raw = Buffer.alloc(size * stride);
  const half = SIZE / 2;
  const scale = SIZE / size;          // device px -> master units
  const step = 1 / SS, base = step / 2;
  const n = SS * SS;

  for (let y = 0; y < size; y += 1) {
    const row = y * stride;
    raw[row] = 0; // PNG filter type: none
    for (let x = 0; x < size; x += 1) {
      let tile = 0, glyph = 0;
      for (let sy = 0; sy < SS; sy += 1) {
        const py = (y + base + sy * step) * scale;
        for (let sx = 0; sx < SS; sx += 1) {
          const px = (x + base + sx * step) * scale;
          if (sdRoundBox(px - half, py - half, half, half, TILE_R) <= 0) tile += 1;
          if (glyphDist(px, py) <= 0) glyph += 1;
        }
      }
      const at = tile / n;
      const ag = (glyph / n) * at;   // glyph never spills past the tile
      const offset = row + 1 + x * 4;
      for (let c = 0; c < 3; c += 1) {
        const col = at > 0 ? (BG[c] * (at - ag) + FG[c] * ag) / at : 0;
        raw[offset + c] = Math.round(clamp(col, 0, 255));
      }
      raw[offset + 3] = Math.round(at * 255);
    }
  }
  return raw;
}

// ── PNG container ──────────────────────────────────────────────────────────

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const name = Buffer.from(type, 'ascii');
  const header = Buffer.alloc(4);
  header.writeUInt32BE(data.length);
  const checksum = Buffer.alloc(4);
  checksum.writeUInt32BE(crc32(Buffer.concat([name, data])));
  return Buffer.concat([header, name, data, checksum]);
}

function encodePng(size) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;   // bit depth
  ihdr[9] = 6;   // colour type: RGBA
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(render(size), { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

for (const [name, size] of [
  ['icon-master.png', SIZE],
  ['512x512.png', 512],
  ['256x256.png', 256],
]) {
  const png = encodePng(size);
  writeFileSync(new URL(`../icons/${name}`, import.meta.url), png);
  console.log(`icons/${name}: ${size}x${size}, ${png.length} bytes`);
}
