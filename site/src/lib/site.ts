import { definePowderworksSite } from "@thepowderworks/fumadocs/config";

export const site = definePowderworksSite({
  name: "Ordnung",
  description: "Repository order, made explicit.",
  repository: "ThePowderworks/ordnung",
  branch: "main",
  siteUrl: process.env.NEXT_PUBLIC_SITE_URL ?? "https://ordnung.powderworks.dev",
  locales: [{ code: "en", name: "English", searchLanguage: "english" }],
  defaultLocale: "en",
  links: [
    { text: "Docs", url: "/docs" },
    { text: "Design", url: "/docs/explanation/model" },
  ],
});
