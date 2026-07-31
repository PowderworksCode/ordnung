import { definePowderworksI18n } from "@thepowderworks/fumadocs/i18n";
import { site } from "./site";

export const { i18n, translations } = definePowderworksI18n(
  site.locales,
  site.defaultLocale,
);

