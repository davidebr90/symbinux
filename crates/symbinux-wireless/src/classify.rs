//! Device classification from vanilla identification signals.
//!
//! Combines the signals a stock device gives away for free — Class of
//! Device, BlueZ icon names, BLE manufacturer company identifiers,
//! advertised service UUIDs and the device name — into a vendor + form
//! factor pair the UI can render. See `docs/VANILLA_CONNECTIVITY.md` §4
//! for the reliability ordering these functions follow.

/// The ecosystem/brand a device most likely belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vendor {
    Apple,
    Google,
    Samsung,
    Microsoft,
    Huawei,
    Xiaomi,
    Sony,
    Garmin,
    Nokia,
    #[default]
    Unknown,
}

impl Vendor {
    /// Short human-readable label, `None` when unknown.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Apple => Some("Apple"),
            Self::Google => Some("Google"),
            Self::Samsung => Some("Samsung"),
            Self::Microsoft => Some("Microsoft"),
            Self::Huawei => Some("Huawei"),
            Self::Xiaomi => Some("Xiaomi"),
            Self::Sony => Some("Sony"),
            Self::Garmin => Some("Garmin"),
            Self::Nokia => Some("Nokia"),
            Self::Unknown => None,
        }
    }

    /// Whether the vendor ships Android-ecosystem phones (drives the robot
    /// badge in the UI when the form factor is a phone).
    pub fn is_android_ecosystem(self) -> bool {
        matches!(
            self,
            Self::Google | Self::Samsung | Self::Huawei | Self::Xiaomi | Self::Sony
        )
    }
}

/// The physical form factor of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    Smartphone,
    /// A phone that is not (or not known to be) a smartphone.
    Phone,
    Watch,
    Computer,
    Tv,
    Audio,
    MediaPlayer,
    Peripheral,
    #[default]
    Unknown,
}

impl Kind {
    /// Short human-readable label, `None` when unknown.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Smartphone => Some("smartphone"),
            Self::Phone => Some("phone"),
            Self::Watch => Some("smartwatch"),
            Self::Computer => Some("computer"),
            Self::Tv => Some("TV"),
            Self::Audio => Some("audio"),
            Self::MediaPlayer => Some("media player"),
            Self::Peripheral => Some("peripheral"),
            Self::Unknown => None,
        }
    }
}

/// Bluetooth SIG company identifier → vendor (BLE manufacturer data, or a
/// `bluetooth:vXXXX…` modalias).
pub(crate) fn vendor_from_company_id(id: u16) -> Vendor {
    match id {
        0x0001 => Vendor::Nokia,
        0x0006 => Vendor::Microsoft,
        0x004C => Vendor::Apple,
        0x0075 => Vendor::Samsung,
        0x0087 => Vendor::Garmin,
        0x00E0 => Vendor::Google,
        0x012D => Vendor::Sony,
        0x027D => Vendor::Huawei,
        0x038F => Vendor::Xiaomi,
        _ => Vendor::Unknown,
    }
}

/// Device-name heuristics for the vendor. Weakest signal — used last.
pub(crate) fn vendor_from_name(name: &str) -> Vendor {
    let name = name.to_lowercase();
    let matches_word = |needle: &str| name.contains(needle);
    if ["iphone", "ipad", "macbook", "imac", "airpods", "apple"]
        .iter()
        .any(|n| matches_word(n))
    {
        Vendor::Apple
    } else if matches_word("galaxy") || matches_word("samsung") {
        Vendor::Samsung
    } else if matches_word("pixel") || matches_word("chromecast") || matches_word("google") {
        Vendor::Google
    } else if matches_word("huawei") || matches_word("honor") {
        Vendor::Huawei
    } else if matches_word("xiaomi") || matches_word("redmi") || matches_word("mi band") {
        Vendor::Xiaomi
    } else if matches_word("sony") || matches_word("wh-") || matches_word("wf-") {
        Vendor::Sony
    } else if matches_word("garmin") {
        Vendor::Garmin
    } else if matches_word("nokia") {
        Vendor::Nokia
    } else if matches_word("surface") || matches_word("microsoft") {
        Vendor::Microsoft
    } else {
        Vendor::Unknown
    }
}

/// Class of Device (24-bit CoD from a classic inquiry) → form factor.
/// Major class = bits 8–12, minor class = bits 2–7.
pub(crate) fn kind_from_cod(cod: u32) -> Kind {
    let major = (cod >> 8) & 0x1F;
    let minor = (cod >> 2) & 0x3F;
    match major {
        1 => Kind::Computer,
        2 => match minor {
            3 => Kind::Smartphone,
            _ => Kind::Phone,
        },
        4 => match minor {
            // Audio/Video minors: 9 set-top box, 11 VCR, 14 video monitor,
            // 15 video display & loudspeaker → TV-like; the rest is audio
            // gear (headset/headphones/portable/car/hi-fi/speaker).
            9 | 11 | 14 | 15 => Kind::Tv,
            _ => Kind::Audio,
        },
        5 => Kind::Peripheral,
        // Wearable major: minor 1 is the watch; treat the whole class as
        // wrist-adjacent for the UI.
        7 => Kind::Watch,
        _ => Kind::Unknown,
    }
}

/// BlueZ `Icon` property (freedesktop icon name) → form factor.
///
/// Only the Linux backend reads BlueZ; keep it compiled and unit-tested
/// everywhere.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn kind_from_bluez_icon(icon: &str) -> Kind {
    match icon {
        "phone" => Kind::Smartphone,
        "watch" => Kind::Watch,
        "computer" => Kind::Computer,
        "video-display" | "tv" => Kind::Tv,
        "audio-headset" | "audio-headphones" | "audio-card" | "audio-speakers" => Kind::Audio,
        "multimedia-player" => Kind::MediaPlayer,
        "input-mouse" | "input-keyboard" | "input-gaming" | "input-tablet" => Kind::Peripheral,
        _ => Kind::Unknown,
    }
}

/// 16-bit advertised service UUIDs → form factor hint.
///
/// Only the BLE backend (Windows/macOS) feeds advertised services today;
/// keep it compiled and unit-tested everywhere.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub(crate) fn kind_from_service_ids(services: &[u16]) -> Kind {
    if services.contains(&0x180D) {
        // Heart Rate → wearable band/watch.
        Kind::Watch
    } else if services.contains(&0x1812) {
        // HID → keyboard/mouse/remote.
        Kind::Peripheral
    } else {
        Kind::Unknown
    }
}

/// Device-name heuristics for the form factor. Weakest signal — used last.
pub(crate) fn kind_from_name(name: &str) -> Kind {
    let name = name.to_lowercase();
    let has = |needle: &str| name.contains(needle);
    if has("watch") || has("band") || has("gt ") || has("amazfit") {
        Kind::Watch
    } else if has(" tv") || name.starts_with("[tv]") || has("bravia") || has("webos") {
        Kind::Tv
    } else if has("iphone") || has("galaxy s") || has("pixel") || has("phone") {
        Kind::Smartphone
    } else if has("buds") || has("airpods") || has("headphone") || has("speaker") || has("soundbar")
    {
        Kind::Audio
    } else if has("macbook") || has("laptop") || has("desktop") || has("-pc") || has("book") {
        Kind::Computer
    } else if has("keyboard") || has("mouse") {
        Kind::Peripheral
    } else {
        Kind::Unknown
    }
}

/// First classification that is not `Unknown`, in the order given.
pub(crate) fn first_known_kind(candidates: &[Kind]) -> Kind {
    candidates
        .iter()
        .copied()
        .find(|kind| *kind != Kind::Unknown)
        .unwrap_or_default()
}

pub(crate) fn first_known_vendor(candidates: &[Vendor]) -> Vendor {
    candidates
        .iter()
        .copied()
        .find(|vendor| *vendor != Vendor::Unknown)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cod_smartphone_and_watch() {
        // Major 2 (phone), minor 3 (smartphone): 0b10_00001100 = 0x20C.
        assert_eq!(kind_from_cod(0x5A020C), Kind::Smartphone);
        // Major 7 (wearable), minor 1 (watch): 0x704.
        assert_eq!(kind_from_cod(0x000704), Kind::Watch);
        // Major 1 (computer).
        assert_eq!(kind_from_cod(0x00010C), Kind::Computer);
    }

    #[test]
    fn company_ids_map_to_vendors() {
        assert_eq!(vendor_from_company_id(0x004C), Vendor::Apple);
        assert_eq!(vendor_from_company_id(0x00E0), Vendor::Google);
        assert_eq!(vendor_from_company_id(0x0001), Vendor::Nokia);
        assert_eq!(vendor_from_company_id(0xFFFF), Vendor::Unknown);
    }

    #[test]
    fn name_heuristics() {
        assert_eq!(vendor_from_name("iPhone di Davide"), Vendor::Apple);
        assert_eq!(kind_from_name("iPhone di Davide"), Kind::Smartphone);
        assert_eq!(kind_from_name("[LG] webOS TV UN73006LA"), Kind::Tv);
        assert_eq!(kind_from_name("Galaxy Watch4"), Kind::Watch);
        assert_eq!(vendor_from_name("Galaxy Watch4"), Vendor::Samsung);
        assert_eq!(kind_from_name("Nuki_3A3C41EC"), Kind::Unknown);
    }

    #[test]
    fn services_and_priorities() {
        assert_eq!(kind_from_service_ids(&[0x180D]), Kind::Watch);
        assert_eq!(kind_from_service_ids(&[0x1812]), Kind::Peripheral);
        assert_eq!(kind_from_service_ids(&[0x180F]), Kind::Unknown);
        assert_eq!(
            first_known_kind(&[Kind::Unknown, Kind::Watch, Kind::Smartphone]),
            Kind::Watch
        );
        assert_eq!(
            first_known_vendor(&[Vendor::Unknown, Vendor::Apple]),
            Vendor::Apple
        );
    }
}
