import { createPowderworksBaseOptions } from "@thepowderworks/fumadocs/layout";
import { site } from "./site";

export function baseOptions(locale: string) {
  return createPowderworksBaseOptions(site, locale);
}

