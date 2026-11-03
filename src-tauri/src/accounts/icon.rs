//! Account icon values, compatible with ZapZap's persisted `users.icon`.
//!
//! An icon is SVG markup (the generated default icon), a
//! `data:image/png;base64,` photo, or a profile that keeps both choices:
//! `zapzap-account-image:v1:{"type":"icon"|"photo","icon":<svg>,"photo":<png>}`.

use serde::{Deserialize, Serialize};

pub const PHOTO_PREFIX: &str = "data:image/png;base64,";
pub const PROFILE_PREFIX: &str = "zapzap-account-image:v1:";

#[derive(Serialize, Deserialize)]
struct Profile {
    #[serde(rename = "type")]
    kind: String,
    icon: String,
    photo: String,
}

fn profile(icon: &str) -> Option<Profile> {
    let profile: Profile = serde_json::from_str(icon.strip_prefix(PROFILE_PREFIX)?).ok()?;
    let valid = matches!(profile.kind.as_str(), "icon" | "photo")
        && profile.photo.starts_with(PHOTO_PREFIX);
    valid.then_some(profile)
}

/// The generated icon retained by a stored value, if any.
fn default_icon(icon: Option<&str>) -> Option<String> {
    let icon = icon.filter(|i| !i.is_empty())?;
    if let Some(profile) = profile(icon) {
        return Some(profile.icon);
    }
    (!icon.starts_with(PROFILE_PREFIX) && !icon.starts_with(PHOTO_PREFIX)).then(|| icon.into())
}

/// Store `photo` as the selected image while keeping the generated icon, so
/// the user can switch back to it as in ZapZap.
pub fn with_photo(current: Option<&str>, photo: String) -> String {
    match default_icon(current) {
        Some(icon) => {
            let profile = Profile {
                kind: "photo".into(),
                icon,
                photo,
            };
            format!(
                "{PROFILE_PREFIX}{}",
                serde_json::to_string(&profile).expect("profile serializes")
            )
        }
        None => photo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,AAAA";

    #[test]
    fn photo_keeps_the_generated_icon() {
        let value = with_photo(Some("<svg/>"), PNG.into());
        let profile = profile(&value).unwrap();
        assert_eq!(profile.kind, "photo");
        assert_eq!(profile.icon, "<svg/>");
        assert_eq!(profile.photo, PNG);
        // Replacing the photo of a profile keeps the same icon.
        let again = profile_icon(&with_photo(Some(&value), PNG.into()));
        assert_eq!(again.as_deref(), Some("<svg/>"));
    }

    fn profile_icon(value: &str) -> Option<String> {
        profile(value).map(|p| p.icon)
    }

    #[test]
    fn photo_without_icon_is_stored_raw() {
        assert_eq!(with_photo(None, PNG.into()), PNG);
        assert_eq!(with_photo(Some(PNG), PNG.into()), PNG);
        assert_eq!(
            with_photo(Some("zapzap-account-image:v1:{bad"), PNG.into()),
            PNG
        );
    }
}
