// Interface translations. Catalogs are generated from po/*.po by
// scripts/po2json.mjs; source strings are English, as in ZapZap.
import { compilePlural } from "./plural";

interface Catalog {
  plural: string;
  messages: Record<string, string>;
  plurals: Record<string, string[]>;
}

const catalogs = import.meta.glob<Catalog>("./catalogs/*.json", {
  eager: true,
  import: "default",
});

export const languages: string[] = Object.keys(catalogs)
  .map((path) => path.slice("./catalogs/".length, -".json".length))
  .sort();

export function catalogCode(locale: string): string | null {
  const lang = locale.split("_")[0];
  return (
    languages.find((code) => code === locale) ??
    languages.find((code) => code === lang) ??
    languages.find((code) => code.split("_")[0] === lang) ??
    null
  );
}

let active = $state<{ code: string | null; catalog: Catalog | null; plural: (n: number) => number }>({
  code: null,
  catalog: null,
  plural: (n) => +(n !== 1),
});

/// Activate the catalog for an `ll_CC` locale (falls back to English).
export function setLocale(locale: string) {
  const code = catalogCode(locale);
  const catalog = code ? catalogs[`./catalogs/${code}.json`] : null;
  active = {
    code,
    catalog,
    plural: compilePlural(catalog?.plural ?? "n != 1"),
  };
  if (typeof document !== "undefined") document.documentElement.lang = (code ?? "en").replace("_", "-");
}

export function currentLocale(): string | null {
  return active.code;
}

function format(text: string, params?: Record<string, string | number>): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, key) => (key in params ? String(params[key]) : match));
}

/// Translate an English source string; `{name}` placeholders take `params`.
export function t(source: string, params?: Record<string, string | number>): string {
  return format(active.catalog?.messages[source] || source, params);
}

/// Plural-aware translation; `{count}` is filled with `n` unless given.
export function tn(
  singular: string,
  plural: string,
  n: number,
  params?: Record<string, string | number>,
): string {
  const forms = active.catalog?.plurals[singular];
  const text = forms ? (forms[active.plural(n)] ?? forms[0]) : n === 1 ? singular : plural;
  return format(text, { count: n, ...params });
}

/// Display name of a catalog code in the current interface language.
/** BCP 47 tag of the interface language, for `Intl` formatters. */
export function uiLocale(): string {
  return (active.code ?? "en").replace("_", "-");
}

export function languageName(code: string): string {
  try {
    const names = new Intl.DisplayNames([(active.code ?? "en").replace("_", "-")], {
      type: "language",
    });
    return names.of(code.replace("_", "-")) ?? code;
  } catch {
    return code;
  }
}
