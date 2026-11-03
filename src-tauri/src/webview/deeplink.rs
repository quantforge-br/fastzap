//! WhatsApp deep links and "open chat by number" validation, ported from
//! ZapZap's `deeplink.py` and `open_chat.py`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use url::Url;

/// ISO 3166-1 region → E.164 calling code, generated from google/libphonenumber
/// metadata (Apache-2.0) as in ZapZap.
const REGION_CALLING_CODE_DATA: &str = "
AC:247 AD:376 AE:971 AF:93 AG:1 AI:1
AL:355 AM:374 AO:244 AR:54 AS:1 AT:43
AU:61 AW:297 AX:358 AZ:994 BA:387 BB:1
BD:880 BE:32 BF:226 BG:359 BH:973 BI:257
BJ:229 BL:590 BM:1 BN:673 BO:591 BQ:599
BR:55 BS:1 BT:975 BW:267 BY:375 BZ:501
CA:1 CC:61 CD:243 CF:236 CG:242 CH:41
CI:225 CK:682 CL:56 CM:237 CN:86 CO:57
CR:506 CU:53 CV:238 CW:599 CX:61 CY:357
CZ:420 DE:49 DJ:253 DK:45 DM:1 DO:1
DZ:213 EC:593 EE:372 EG:20 EH:212 ER:291
ES:34 ET:251 FI:358 FJ:679 FK:500 FM:691
FO:298 FR:33 GA:241 GB:44 GD:1 GE:995
GF:594 GG:44 GH:233 GI:350 GL:299 GM:220
GN:224 GP:590 GQ:240 GR:30 GT:502 GU:1
GW:245 GY:592 HK:852 HN:504 HR:385 HT:509
HU:36 ID:62 IE:353 IL:972 IM:44 IN:91
IO:246 IQ:964 IR:98 IS:354 IT:39 JE:44
JM:1 JO:962 JP:81 KE:254 KG:996 KH:855
KI:686 KM:269 KN:1 KP:850 KR:82 KW:965
KY:1 KZ:7 LA:856 LB:961 LC:1 LI:423
LK:94 LR:231 LS:266 LT:370 LU:352 LV:371
LY:218 MA:212 MC:377 MD:373 ME:382 MF:590
MG:261 MH:692 MK:389 ML:223 MM:95 MN:976
MO:853 MP:1 MQ:596 MR:222 MS:1 MT:356
MU:230 MV:960 MW:265 MX:52 MY:60 MZ:258
NA:264 NC:687 NE:227 NF:672 NG:234 NI:505
NL:31 NO:47 NP:977 NR:674 NU:683 NZ:64
OM:968 PA:507 PE:51 PF:689 PG:675 PH:63
PK:92 PL:48 PM:508 PR:1 PS:970 PT:351
PW:680 PY:595 QA:974 RE:262 RO:40 RS:381
RU:7 RW:250 SA:966 SB:677 SC:248 SD:249
SE:46 SG:65 SH:290 SI:386 SJ:47 SK:421
SL:232 SM:378 SN:221 SO:252 SR:597 SS:211
ST:239 SV:503 SX:1 SY:963 SZ:268 TA:290
TC:1 TD:235 TG:228 TH:66 TJ:992 TK:690
TL:670 TM:993 TN:216 TO:676 TR:90 TT:1
TV:688 TW:886 TZ:255 UA:380 UG:256 US:1
UY:598 UZ:998 VA:39 VC:1 VE:58 VG:1
VI:1 VN:84 VU:678 WF:681 WS:685 XK:383
YE:967 YT:262 ZA:27 ZM:260 ZW:263
";

const ALLOWED_HTTPS_HOSTS: &[&str] = &[
    "api.whatsapp.com",
    "web.whatsapp.com",
    "whatsapp.com",
    "wa.me",
];
pub const MIN_E164_DIGITS: usize = 7;
pub const MAX_E164_DIGITS: usize = 15;
pub const MAX_MESSAGE_LENGTH: usize = 500;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct RegionCallingCode {
    pub region: String,
    pub calling_code: String,
}

pub fn region_calling_codes() -> Vec<RegionCallingCode> {
    REGION_CALLING_CODE_DATA
        .split_whitespace()
        .filter_map(|item| item.split_once(':'))
        .map(|(region, code)| RegionCallingCode {
            region: region.to_owned(),
            calling_code: code.to_owned(),
        })
        .collect()
}

fn is_valid_calling_code(code: &str) -> bool {
    REGION_CALLING_CODE_DATA
        .split_whitespace()
        .filter_map(|item| item.split_once(':'))
        .any(|(_, c)| c == code)
}

fn is_allowed_https_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    ALLOWED_HTTPS_HOSTS.contains(&host.as_str()) || host.ends_with(".whatsapp.com")
}

/// Return the deep link if it may be opened inside WhatsApp Web.
pub fn validate_deep_link(raw: &str) -> Option<Url> {
    let url = Url::parse(raw.trim()).ok()?;
    match url.scheme() {
        "whatsapp" => Some(url),
        "https" => {
            if !url.username().is_empty() || url.password().is_some() {
                return None;
            }
            is_allowed_https_host(url.host_str()?).then_some(url)
        }
        _ => None,
    }
}

/// JavaScript that opens an allowed deep link by clicking a temporary anchor.
/// The URL is embedded as a JSON string literal so it cannot escape the call.
pub fn open_deep_link_script(raw: &str) -> Option<String> {
    let url = validate_deep_link(raw)?;
    let href = serde_json::to_string(url.as_str()).ok()?;
    Some(format!(
        "window.__FASTZAP__ && window.__FASTZAP__.openDeepLink({href});"
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ChatTargetError {
    #[error("missing country")]
    MissingCountry,
    #[error("empty number")]
    EmptyNumber,
    #[error("number too short")]
    NumberTooShort,
    #[error("number too long")]
    NumberTooLong,
    #[error("duplicated country code")]
    DuplicatedCountryCode,
    #[error("message too long")]
    MessageTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ChatTarget {
    pub country_code: String,
    pub national_number: String,
    pub message: String,
}

fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

pub fn validate_chat_target(input: &ChatTarget) -> Result<ChatTarget, ChatTargetError> {
    let calling_code = digits(&input.country_code);
    if !is_valid_calling_code(&calling_code) {
        return Err(ChatTargetError::MissingCountry);
    }
    let number = digits(&input.national_number);
    if number.is_empty() {
        return Err(ChatTargetError::EmptyNumber);
    }
    let trimmed = input.national_number.trim_start();
    if trimmed.starts_with('+') || trimmed.starts_with("00") {
        return Err(ChatTargetError::DuplicatedCountryCode);
    }
    let total = calling_code.len() + number.len();
    if total < MIN_E164_DIGITS {
        return Err(ChatTargetError::NumberTooShort);
    }
    if total > MAX_E164_DIGITS {
        return Err(ChatTargetError::NumberTooLong);
    }
    if input.message.chars().count() > MAX_MESSAGE_LENGTH {
        return Err(ChatTargetError::MessageTooLong);
    }
    Ok(ChatTarget {
        country_code: calling_code,
        national_number: number,
        message: input.message.clone(),
    })
}

pub fn open_chat_url(target: &ChatTarget) -> Url {
    let mut url = Url::parse("https://web.whatsapp.com/send").expect("static URL");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair(
            "phone",
            &format!("{}{}", target.country_code, target.national_number),
        );
        if !target.message.is_empty() {
            query.append_pair("text", &target.message);
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_link_hosts_are_restricted() {
        assert!(validate_deep_link("whatsapp://send?phone=5511999999999").is_some());
        assert!(validate_deep_link("https://wa.me/5511999999999").is_some());
        assert!(validate_deep_link("https://chat.whatsapp.com/abc").is_some());
        assert!(validate_deep_link("https://evil.example/wa.me").is_none());
        assert!(validate_deep_link("https://user:pw@wa.me/1").is_none());
        assert!(validate_deep_link("javascript:alert(1)").is_none());
        assert!(validate_deep_link("http://wa.me/1").is_none());
    }

    #[test]
    fn deep_link_script_escapes_url() {
        let script = open_deep_link_script("whatsapp://send?text=%22);alert(1)//").unwrap();
        assert!(script.contains(r#"openDeepLink("whatsapp://send?text=%22);alert(1)//")"#));
    }

    fn target(cc: &str, number: &str, message: &str) -> ChatTarget {
        ChatTarget {
            country_code: cc.into(),
            national_number: number.into(),
            message: message.into(),
        }
    }

    #[test]
    fn chat_target_validation_matches_zapzap() {
        let ok = validate_chat_target(&target("+55", "(11) 99999-9999", "oi")).unwrap();
        assert_eq!(ok.country_code, "55");
        assert_eq!(ok.national_number, "11999999999");
        assert_eq!(
            open_chat_url(&ok).as_str(),
            "https://web.whatsapp.com/send?phone=5511999999999&text=oi"
        );
        assert_eq!(
            validate_chat_target(&target("999", "1234567", "")),
            Err(ChatTargetError::MissingCountry)
        );
        assert_eq!(
            validate_chat_target(&target("55", "", "")),
            Err(ChatTargetError::EmptyNumber)
        );
        assert_eq!(
            validate_chat_target(&target("55", "+55 11", "")),
            Err(ChatTargetError::DuplicatedCountryCode)
        );
        assert_eq!(
            validate_chat_target(&target("55", "123", "")),
            Err(ChatTargetError::NumberTooShort)
        );
        assert_eq!(
            validate_chat_target(&target("55", "1234567890123456", "")),
            Err(ChatTargetError::NumberTooLong)
        );
        assert_eq!(
            validate_chat_target(&target("55", "11999999999", &"x".repeat(501))),
            Err(ChatTargetError::MessageTooLong)
        );
    }
}
