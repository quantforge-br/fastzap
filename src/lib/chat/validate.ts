// Live feedback for the "open chat by number" dialog; the backend applies
// the same rules (webview::deeplink::validate_chat_target) before opening.
export const MIN_E164_DIGITS = 7;
export const MAX_E164_DIGITS = 15;
export const MAX_MESSAGE_LENGTH = 500;

export type ChatError =
  | "missing_country"
  | "empty_number"
  | "number_too_short"
  | "number_too_long"
  | "duplicated_country_code"
  | "message_too_long";

const digits = (value: string) => value.replace(/\D/g, "");

export function validateChat(countryCode: string, number: string, message: string): ChatError | null {
  const code = digits(countryCode);
  if (!code) return "missing_country";
  const national = digits(number);
  if (!national) return "empty_number";
  const trimmed = number.trimStart();
  if (trimmed.startsWith("+") || trimmed.startsWith("00")) return "duplicated_country_code";
  const total = code.length + national.length;
  if (total < MIN_E164_DIGITS) return "number_too_short";
  if (total > MAX_E164_DIGITS) return "number_too_long";
  if ([...message].length > MAX_MESSAGE_LENGTH) return "message_too_long";
  return null;
}

/** The region of the system or interface locale (`pt-BR` → `BR`). */
export function defaultRegion(locales: readonly string[]): string | null {
  for (const locale of locales) {
    try {
      const region = new Intl.Locale(locale).maximize().region;
      if (region) return region;
    } catch {
      // Ignore malformed tags.
    }
  }
  return null;
}
