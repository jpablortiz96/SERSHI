// Stages SERSHI's inference engine for installer builds (Prompt 4).
//
// Tauri bundles "external binaries" from src-tauri/binaries/<name>-<target
// triple><ext>. This copies the release build of `sershi-semantic` there.
// It runs only for installer builds (tauri.bundle.conf.json); ordinary
// builds, `pnpm dev` and CI never need it.
//
// Honours CARGO_TARGET_DIR. No downloads, no network: the engine is built
// from this repository by the same build.
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../..");
const targetDir = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(repo, "target");
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" })
  .split("\n")
  .find((line) => line.startsWith("host:"))
  ?.slice("host:".length)
  .trim();
if (!host) {
  console.error("stage-sidecars: could not read the Rust host triple");
  process.exit(1);
}
const ext = host.includes("windows") ? ".exe" : "";
const source = join(targetDir, "release", `sershi-semantic${ext}`);
if (!existsSync(source)) {
  console.error(`stage-sidecars: ${source} is missing; build it first:`);
  console.error("  cargo build -p sershi-semantic --release --locked");
  process.exit(1);
}
const dest = join(here, "..", "src-tauri", "binaries", `sershi-semantic-${host}${ext}`);
mkdirSync(dirname(dest), { recursive: true });
copyFileSync(source, dest);
console.log(`stage-sidecars: ${source} -> ${dest}`);
