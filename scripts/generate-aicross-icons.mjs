import { writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const iconsDir = new URL('../icons/', import.meta.url);

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) c = (c & 1) ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
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

const glyphs = {
  A: ['01110', '10001', '10001', '11111', '10001', '10001', '10001'],
  C: ['01111', '10000', '10000', '10000', '10000', '10000', '01111'],
};

function makePng(size) {
  const rgba = Buffer.alloc(size * size * 4);
  const radius = size * 0.22;
  const low = radius;
  const high = size - radius;

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const px = x + 0.5;
      const py = y + 0.5;
      const cx = Math.max(low, Math.min(high, px));
      const cy = Math.max(low, Math.min(high, py));
      const distance = Math.hypot(px - cx, py - cy);
      const alpha = Math.max(0, Math.min(1, radius + 0.5 - distance));
      const offset = (y * size + x) * 4;
      rgba[offset] = 24;
      rgba[offset + 1] = 33;
      rgba[offset + 2] = 44;
      rgba[offset + 3] = Math.round(alpha * 255);
    }
  }

  const cell = Math.max(1, Math.floor(size * 0.055));
  const totalWidth = 11 * cell;
  const totalHeight = 7 * cell;
  const startX = Math.floor((size - totalWidth) / 2);
  const startY = Math.floor((size - totalHeight) / 2);

  for (const [glyphIndex, letter] of ['A', 'C'].entries()) {
    for (let row = 0; row < 7; row += 1) {
      for (let col = 0; col < 5; col += 1) {
        if (glyphs[letter][row][col] !== '1') continue;
        const left = startX + glyphIndex * 6 * cell + col * cell;
        const top = startY + row * cell;
        for (let dy = 0; dy < cell; dy += 1) {
          for (let dx = 0; dx < cell; dx += 1) {
            const x = left + dx;
            const y = top + dy;
            if (x < 0 || y < 0 || x >= size || y >= size) continue;
            const offset = (y * size + x) * 4;
            rgba[offset] = 255;
            rgba[offset + 1] = 255;
            rgba[offset + 2] = 255;
            rgba[offset + 3] = 255;
          }
        }
      }
    }
  }

  const scanlines = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    const rowStart = y * (size * 4 + 1);
    scanlines[rowStart] = 0;
    rgba.copy(scanlines, rowStart + 1, y * size * 4, (y + 1) * size * 4);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(scanlines, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const pngFiles = new Map([
  ['32x32.png', 32],
  ['64x64.png', 64],
  ['128x128.png', 128],
  ['128x128@2x.png', 256],
  ['256x256.png', 256],
  ['512x512.png', 512],
  ['icon.png', 512],
  ['Square30x30Logo.png', 30],
  ['Square44x44Logo.png', 44],
  ['Square71x71Logo.png', 71],
  ['Square89x89Logo.png', 89],
  ['Square107x107Logo.png', 107],
  ['Square142x142Logo.png', 142],
  ['Square150x150Logo.png', 150],
  ['Square284x284Logo.png', 284],
  ['Square310x310Logo.png', 310],
  ['StoreLogo.png', 50],
]);

for (const [name, size] of pngFiles) {
  writeFileSync(new URL(name, iconsDir), makePng(size));
  console.log(`${name}: ${size}x${size}`);
}

const icoSizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
const icoImages = icoSizes.map(makePng);
const icoHeader = Buffer.alloc(6 + icoSizes.length * 16);
icoHeader.writeUInt16LE(0, 0);
icoHeader.writeUInt16LE(1, 2);
icoHeader.writeUInt16LE(icoSizes.length, 4);
let icoOffset = icoHeader.length;
for (let i = 0; i < icoSizes.length; i += 1) {
  const entry = 6 + i * 16;
  const size = icoSizes[i];
  icoHeader[entry] = size === 256 ? 0 : size;
  icoHeader[entry + 1] = size === 256 ? 0 : size;
  icoHeader.writeUInt16LE(1, entry + 4);
  icoHeader.writeUInt16LE(32, entry + 6);
  icoHeader.writeUInt32LE(icoImages[i].length, entry + 8);
  icoHeader.writeUInt32LE(icoOffset, entry + 12);
  icoOffset += icoImages[i].length;
}
writeFileSync(new URL('icon.ico', iconsDir), Buffer.concat([icoHeader, ...icoImages]));
console.log(`icon.ico: ${icoSizes.join(', ')} px`);

const icnsEntries = [
  ['icp4', 16],
  ['icp5', 32],
  ['icp6', 64],
  ['ic07', 128],
  ['ic08', 256],
  ['ic09', 512],
  ['ic10', 1024],
].map(([type, size]) => {
  const data = makePng(size);
  const header = Buffer.alloc(8);
  header.write(type, 0, 4, 'ascii');
  header.writeUInt32BE(data.length + 8, 4);
  return Buffer.concat([header, data]);
});
const icnsHeader = Buffer.alloc(8);
icnsHeader.write('icns', 0, 4, 'ascii');
icnsHeader.writeUInt32BE(8 + icnsEntries.reduce((sum, item) => sum + item.length, 0), 4);
writeFileSync(new URL('icon.icns', iconsDir), Buffer.concat([icnsHeader, ...icnsEntries]));
console.log('icon.icns: 16, 32, 64, 128, 256, 512, 1024 px');
