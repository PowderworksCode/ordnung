// Checks the documentation against content/checks.json, which is exported
// from the binary by scripts/checks-manifest.sh. The manifest is the contract:
// every check the binary carries must be documented with its real severity,
// and no page may claim a check the binary does not carry.
import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

type Manifest = {
  schema: string;
  version: string;
  checks: {
    id: string;
    summary: string;
    category: string;
    scope: string;
    default_severity: string;
    surfaces: string[];
  }[];
  removed: string[];
};

const ROOT = join(import.meta.dir, "..");
const DOCS_DIR = join(ROOT, "content");
const CHECKS_REFERENCE = join(DOCS_DIR, "reference", "checks.md");

// The export is the CLI's standard JSON envelope; the manifest is its data.
const envelope = JSON.parse(
  readFileSync(join(DOCS_DIR, "checks.json"), "utf8"),
);
const manifest: Manifest = envelope.data;

const liveIds = new Set(manifest.checks.map((check) => check.id));
const removedIds = new Set(manifest.removed);

function markdownFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...markdownFiles(full));
    else if (entry.name.endsWith(".md")) out.push(full);
  }
  return out;
}

const pages = markdownFiles(DOCS_DIR).map((path) => ({
  rel: path.slice(DOCS_DIR.length + 1),
  text: readFileSync(path, "utf8"),
}));

/**
 * Check ids the prose actually asserts are checks, rather than every
 * backticked token that happens to look like one. Two shapes carry that
 * claim: a severity assignment in a TOML config block (`codespell =
 * { severity = "off" }` under [checks] or [overrides]), and the check column
 * of sample `ordnung check` output (`fail  required  codespell  ...`).
 */
function claimedCheckIds(text: string): Map<string, string> {
  const claims = new Map<string, string>();
  const add = (id: string, context: string) => {
    if (!claims.has(id)) claims.set(id, context);
  };

  for (const block of text.matchAll(/```[a-z]*\n([\s\S]*?)```/g)) {
    const body = block[1] ?? "";
    let inCheckTable = false;
    for (const line of body.split("\n")) {
      const section = /^\s*\[{1,2}([a-z_-]+)\]{1,2}\s*$/.exec(line)?.[1];
      if (section) inCheckTable = ["checks", "overrides"].includes(section);
      else if (inCheckTable) {
        const assignment = /^\s*([a-z][a-z0-9-]*)\s*=/.exec(line)?.[1];
        if (assignment) add(assignment, `config assignment ${assignment}`);
      }
      const output =
        /^(?:pass|fail|skip|error)\s+(?:required|recommended|off)\s+([a-z][a-z0-9-]*)\s/.exec(
          line,
        )?.[1];
      if (output) add(output, `sample output ${output}`);
    }
  }

  return claims;
}

describe("checks manifest", () => {
  test("is the schema these tests understand", () => {
    expect(manifest.schema).toBe("ordnung.checks/1");
    expect(manifest.checks.length).toBeGreaterThan(0);
  });

  test("no check is both live and withdrawn", () => {
    for (const id of removedIds) expect(liveIds.has(id)).toBe(false);
  });
});

describe("every check is documented", () => {
  const reference = readFileSync(CHECKS_REFERENCE, "utf8");

  for (const check of manifest.checks) {
    const row = reference
      .split("\n")
      .find((line) => line.startsWith(`| \`${check.id}\``));

    test(`${check.id} has a row in the checks reference`, () => {
      expect(row, `no table row for ${check.id}`).toBeDefined();
    });

    test(`${check.id} quotes its severity and scope`, () => {
      expect(row).toContain(`| ${check.default_severity}`);
      expect(row).toContain(`| ${check.scope} |`);
    });

    test(`${check.id} sits under its category heading`, () => {
      const section = reference.indexOf(`## ${check.category}`);
      expect(section, `no section for ${check.category}`).toBeGreaterThan(-1);
      const next = reference.indexOf("\n## ", section + 1);
      const body = reference.slice(section, next === -1 ? undefined : next);
      expect(body).toContain(`| \`${check.id}\``);
    });
  }

  test("the reference marks the opt-in checks as opt-in", () => {
    for (const check of manifest.checks.filter(
      (c) => c.default_severity === "off",
    )) {
      const row = reference
        .split("\n")
        .find((line) => line.startsWith(`| \`${check.id}\``));
      expect(row, `no table row for ${check.id}`).toBeDefined();
      expect(row).toContain("opt-in");
    }
  });

  test("the reference documents no check the binary does not carry", () => {
    // A check row has four columns; the severity legend's two-column rows
    // also start with a backticked token and are not claims.
    for (const hit of reference.matchAll(
      /^\| `([a-z][a-z0-9-]*)` \|[^|]+\| (?:repository|project) \|/gm,
    )) {
      const id = hit[1] ?? "";
      expect(liveIds.has(id), `${id} is not an Ordnung check`).toBe(true);
    }
  });
});

describe("no page claims a check that does not exist", () => {
  for (const page of pages) {
    const claims = claimedCheckIds(page.text);
    if (claims.size === 0) continue;

    test(`${page.rel}`, () => {
      const bogus: string[] = [];
      for (const [id, context] of claims) {
        if (liveIds.has(id)) continue;
        const why = removedIds.has(id)
          ? `${id} was withdrawn from Ordnung`
          : `${id} is not an Ordnung check`;
        bogus.push(`${why} (${context})`);
      }
      expect(bogus).toEqual([]);
    });
  }
});

describe("no page mentions a withdrawn check at all", () => {
  for (const page of pages) {
    test(`${page.rel}`, () => {
      const mentioned = [...removedIds].filter((id) =>
        page.text.includes(`\`${id}\``),
      );
      expect(mentioned).toEqual([]);
    });
  }
});
