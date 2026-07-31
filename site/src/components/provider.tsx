"use client";

import { PowderworksProvider } from "@thepowderworks/fumadocs/provider";
import type { ReactNode } from "react";
import SearchDialog from "./search";
import { translations } from "@/lib/i18n";

export function Provider({ lang, children }: { lang: string; children: ReactNode }) {
  return (
    <PowderworksProvider lang={lang} translations={translations} search={{ SearchDialog }}>
      {children}
    </PowderworksProvider>
  );
}

