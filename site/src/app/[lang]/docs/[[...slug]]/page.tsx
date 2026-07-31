import { getMDXComponents } from "@/components/mdx";
import { getPageMarkdownUrl, source } from "@/lib/source";
import { site } from "@/lib/site";
import { localizePath, repositoryUrl } from "@thepowderworks/fumadocs/config";
import { Card } from "fumadocs-ui/components/card";
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
  MarkdownCopyButton,
  ViewOptionsPopover,
} from "fumadocs-ui/layouts/docs/page";
import { createRelativeLink } from "fumadocs-ui/mdx";
import type { Metadata } from "next";
import { notFound } from "next/navigation";

type Params = Promise<{ lang: string; slug?: string[] }>;

export default async function DocsPageRoute({ params }: { params: Params }) {
  const { lang, slug } = await params;
  const page = source.getPage(slug, lang);
  if (!page) notFound();

  const MDX = page.data.body;
  const markdownUrl = getPageMarkdownUrl(page).url;
  const RelativeLink = createRelativeLink(source, page);

  return (
    <DocsPage toc={page.data.toc} full={page.data.full}>
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription className="mb-0">{page.data.description}</DocsDescription>
      <div className="flex items-center gap-2 border-b pb-6">
        <MarkdownCopyButton markdownUrl={markdownUrl} />
        <ViewOptionsPopover
          markdownUrl={markdownUrl}
          githubUrl={`${repositoryUrl(site.repository)}/blob/${site.branch}/site/content/docs/${page.path}`}
        />
      </div>
      <DocsBody>
        <MDX
          components={getMDXComponents({
            a: ({ href, ...props }) => (
              <RelativeLink href={href?.startsWith("/") ? localizePath(lang, href) : href} {...props} />
            ),
            Card: ({ href, ...props }) => (
              <Card href={href?.startsWith("/") ? localizePath(lang, href) : href} {...props} />
            ),
          })}
        />
      </DocsBody>
    </DocsPage>
  );
}

export async function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata({ params }: { params: Params }): Promise<Metadata> {
  const { lang, slug } = await params;
  const page = source.getPage(slug, lang);
  if (!page) notFound();
  return { title: page.data.title, description: page.data.description };
}

