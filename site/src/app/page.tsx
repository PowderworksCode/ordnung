import type { Metadata } from "next";
import { site } from "@/lib/site";

export const metadata: Metadata = { robots: { index: false, follow: false } };

export default function RootRedirect() {
  return (
    <>
      <meta httpEquiv="refresh" content={`0; url=/${site.defaultLocale}`} />
      <main className="mx-auto max-w-3xl px-6 py-24">
        Redirecting to <a href={`/${site.defaultLocale}`}>/{site.defaultLocale}</a>…
      </main>
    </>
  );
}

