import Link from "next/link";
import { i18n } from "@/lib/i18n";

export function generateStaticParams() {
  return i18n.languages.map((lang) => ({ lang }));
}

const checks = [
  ["project-inventory", "pass"],
  ["required-checks", "pass"],
  ["managed-config", "2 changes"],
] as const;

export default async function HomePage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang } = await params;
  return (
    <main className="ordnung-home">
      <section className="ordnung-hero">
        <div className="ordnung-hero-copy">
          <p className="ordnung-kicker">Powderworks field manual · № 01</p>
          <h1>Repository order,<br />made explicit.</h1>
          <p className="ordnung-deck">
            Inspect structure once, resolve policy deterministically, and keep a whole fleet in
            formation without hiding the plan.
          </p>
          <div className="ordnung-actions">
            <Link className="ordnung-primary" href={`/${lang}/docs/tutorials/first-check`}>
              Run the first check
            </Link>
            <Link className="ordnung-secondary" href="https://github.com/ThePowderworks/ordnung">
              Read the source
            </Link>
          </div>
        </div>

        <div className="ordnung-register" aria-label="Example Ordnung check register">
          <div className="ordnung-register-head">
            <span>inspection register</span>
            <span>local / dry-run</span>
          </div>
          <div className="ordnung-command"><span>$</span> ordnung check .</div>
          <dl>
            {checks.map(([name, result], index) => (
              <div key={name}>
                <dt><span>{String(index + 1).padStart(2, "0")}</span>{name}</dt>
                <dd data-result={result}>{result}</dd>
              </div>
            ))}
          </dl>
          <p className="ordnung-register-foot">No mutation without an explicit apply.</p>
        </div>
      </section>

      <section className="ordnung-method" aria-labelledby="method-heading">
        <div>
          <p className="ordnung-section-number">I–III</p>
          <h2 id="method-heading">One inventory.<br />One visible plan.</h2>
        </div>
        <ol>
          <li><span>01</span><div><h3>Inspect</h3><p>Walk the repository once and record typed evidence about projects, packages, workflows, and settings.</p></div></li>
          <li><span>02</span><div><h3>Resolve</h3><p>Combine defaults, fleet policy, and explicitly permitted local exceptions into one effective contract.</p></div></li>
          <li><span>03</span><div><h3>Synchronize</h3><p>Show exact file and GitHub-setting changes before applying anything to a member repository.</p></div></li>
        </ol>
      </section>
    </main>
  );
}
