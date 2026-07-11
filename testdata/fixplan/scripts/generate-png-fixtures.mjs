import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";

const scriptPath = fileURLToPath(import.meta.url);
const fixtureRoot = resolve(dirname(scriptPath), "..", "fixtures", "step07", "images");
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export function generatePngFixture(width, height, rgba = [46, 52, 64, 255]) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1) {
    throw new Error("PNG fixture dimensions must be positive integers");
  }
  const row = Buffer.alloc(1 + width * 4);
  row[0] = 0;
  for (let offset = 1; offset < row.length; offset += 4) {
    row.set(rgba, offset);
  }
  const raw = Buffer.alloc(row.length * height);
  for (let y = 0; y < height; y += 1) {
    row.copy(raw, y * row.length);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  return Buffer.concat([
    pngSignature,
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", deflateSync(raw, { level: 9 })),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

export function readPngDimensions(bytes) {
  const buffer = Buffer.from(bytes);
  if (buffer.length < 24 || !buffer.subarray(0, 8).equals(pngSignature)) {
    throw new Error("invalid PNG fixture signature");
  }
  if (buffer.subarray(12, 16).toString("ascii") !== "IHDR") {
    throw new Error("PNG fixture is missing IHDR");
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

export async function writePngFixtures(root = fixtureRoot) {
  await mkdir(root, { recursive: true });
  const visible = generatePngFixture(640, 384, [46, 52, 64, 255]);
  const legacy = generatePngFixture(1, 1, [0, 0, 0, 0]);
  await Promise.all([
    writeFile(join(root, "visible-640x384.png"), visible),
    writeFile(join(root, "legacy-1x1.png"), legacy),
  ]);
  return { visible: readPngDimensions(visible), legacy: readPngDimensions(legacy) };
}

function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBytes, data])), 0);
  return Buffer.concat([length, typeBytes, data, crc]);
}

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  if (!process.argv.includes("--write")) {
    console.log("Use --write to generate deterministic Step07 PNG fixtures.");
  } else {
    const result = await writePngFixtures();
    console.log(`PNG fixtures written: ${result.visible.width}x${result.visible.height}, ${result.legacy.width}x${result.legacy.height}`);
  }
}

