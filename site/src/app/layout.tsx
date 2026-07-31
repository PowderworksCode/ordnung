import type { Metadata } from "next";
import type { ReactNode } from "react";
import { site } from "@/lib/site";
import "./global.css";

export const metadata: Metadata = {
  metadataBase: new URL(site.siteUrl),
  title: { default: `${site.name} — ${site.description}`, template: `%s · ${site.name}` },
  description: "Inspect repository structure, enforce explicit policy, and synchronize fleet-owned configuration.",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang={site.defaultLocale} suppressHydrationWarning>
      <body className="flex min-h-screen flex-col">{children}</body>
    </html>
  );
}

