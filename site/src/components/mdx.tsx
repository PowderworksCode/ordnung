import { getPowderworksMDXComponents } from "@thepowderworks/fumadocs/mdx";

export const getMDXComponents = getPowderworksMDXComponents;
export const useMDXComponents = getPowderworksMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}

