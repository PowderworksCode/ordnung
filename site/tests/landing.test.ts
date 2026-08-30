// The landing page quotes how many checks Ordnung carries. The number is
// hand-written prose, so this pins it to the manifest the binary exports —
// the same authority the checks reference is tested against — and a check
// added or withdrawn fails here rather than leaving the front page lying.
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const CONTENT_DIR = join(import.meta.dir, "..", "content");

describe("landing page", () => {
  test("the advertised check count matches the manifest", () => {
    const landing = readFileSync(join(CONTENT_DIR, "index.md"), "utf8");
    const claim = /\[(\d+) checks\]\(\/reference\/checks\)/.exec(landing);
    expect(claim).not.toBeNull();
    const manifest = JSON.parse(
      readFileSync(join(CONTENT_DIR, "checks.json"), "utf8"),
    );
    expect(Number(claim?.[1])).toBe(manifest.data.checks.length);
  });
});
