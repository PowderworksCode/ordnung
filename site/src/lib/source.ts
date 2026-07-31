import { docs } from "collections/server";
import { loader } from "fumadocs-core/source";
import { i18n } from "./i18n";

export const source = loader({
  i18n,
  baseUrl: "/docs",
  source: docs.toFumadocsSource(),
});

export function getPageMarkdownUrl(page: (typeof source)["$inferPage"]) {
  const segments = [page.locale, ...page.slugs, "content.md"];
  return {
    segments,
    url: `/llms.mdx/docs/${segments.join("/")}`,
  };
}

export async function getLLMText(page: (typeof source)["$inferPage"]) {
  const processed = await page.data.getText("processed");
  return `# ${page.data.title} (${page.url})\n\n${processed}`;
}
