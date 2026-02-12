/// Known Riot chat server addresses by region.
/// Fallback for when we can't extract it from the config proxy.
pub fn chat_server_for_region(region: &str) -> Option<&'static str> {
    match region.to_lowercase().as_str() {
        "br" | "br1" => Some("br1.chat.si.riotgames.com"),
        "eun" | "eun1" => Some("eun1.chat.si.riotgames.com"),
        "euw" | "euw1" => Some("euw1.chat.si.riotgames.com"),
        "jp" | "jp1" => Some("jp1.chat.si.riotgames.com"),
        "kr" | "kr1" => Some("kr1.chat.si.riotgames.com"),
        "la1" | "lan" => Some("la1.chat.si.riotgames.com"),
        "la2" | "las" => Some("la2.chat.si.riotgames.com"),
        "na" | "na1" | "na2" => Some("na2.chat.si.riotgames.com"),
        "oc" | "oc1" | "oce" => Some("oc1.chat.si.riotgames.com"),
        "ph" | "ph2" => Some("ph2.chat.si.riotgames.com"),
        "ru" | "ru1" => Some("ru1.chat.si.riotgames.com"),
        "sg" | "sg2" => Some("sg2.chat.si.riotgames.com"),
        "th" | "th2" => Some("th2.chat.si.riotgames.com"),
        "tr" | "tr1" => Some("tr1.chat.si.riotgames.com"),
        "tw" | "tw2" => Some("tw2.chat.si.riotgames.com"),
        "vn" | "vn2" => Some("vn2.chat.si.riotgames.com"),
        _ => None,
    }
}

/// List of all known regions for a dropdown selector.
pub const REGIONS: &[(&str, &str)] = &[
    ("br", "Brazil"),
    ("eun", "EU Nordic & East"),
    ("euw", "EU West"),
    ("jp", "Japan"),
    ("kr", "Korea"),
    ("la1", "Latin America North"),
    ("la2", "Latin America South"),
    ("na", "North America"),
    ("oc", "Oceania"),
    ("ph", "Philippines"),
    ("ru", "Russia"),
    ("sg", "Singapore"),
    ("th", "Thailand"),
    ("tr", "Turkey"),
    ("tw", "Taiwan"),
    ("vn", "Vietnam"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_regions() {
        assert_eq!(
            chat_server_for_region("br"),
            Some("br1.chat.si.riotgames.com")
        );
        assert_eq!(
            chat_server_for_region("na"),
            Some("na2.chat.si.riotgames.com")
        );
        assert_eq!(
            chat_server_for_region("euw1"),
            Some("euw1.chat.si.riotgames.com")
        );
    }

    #[test]
    fn test_unknown_region() {
        assert_eq!(chat_server_for_region("unknown"), None);
    }
}
