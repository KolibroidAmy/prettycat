use crate::console::Color;


/// Contains the details of a flag preset
#[derive(Debug, Copy, Clone)]
pub struct FlagPreset {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub stripes: &'static [Color]
}

/// List of built-in preset flags. Currently only includes pride flags at the moment.
/// Most non-pride flags (such as national flags) are probably better suited for image mode anyway.
const FLAG_PRESETS: &[FlagPreset] = &[
    FlagPreset {
        name: "Pride",
        aliases: &["Rainbow"],
        stripes: &hex_sequence([0xE40303, 0xFF8C00, 0xFFED00, 0x008026, 0x24408E, 0x732982]),
    },
    FlagPreset {
        name: "Progress",
        aliases: &[],
        stripes: &hex_sequence([0xE40303, 0xFF8C00, 0xFFED00, 0x008026, 0x24408E, 0x732982, 0x222222, 0x7c3f00, 0x5BCEFA, 0xF5A9B8, 0xFFFFFF]),
    },
    // "Sapphic" has a separate flag - should the stripes for this flag be added? (perhaps ignoring the flowers)
    FlagPreset {
        name: "Lesbian",
        aliases: &[],
        stripes: &hex_sequence([0xD52D00, 0xEF7627, 0xFF9A56, 0xFFFFFF, 0xD162A4, 0xB55690, 0xA30262]),
    },
    FlagPreset {
        name: "Gay",
        aliases: &[],
        stripes: &hex_sequence([0x078D70, 0x26CEAA, 0x98E8C1, 0xFFFFFF, 0x7BADE2, 0x5049CC, 0x3D1A78]),
    },
    FlagPreset {
        name: "Bi",
        aliases: &["Bisexual"],
        stripes: &hex_sequence([0xD60270, 0xD60270, 0x9B4F96, 0x0038A8, 0x0038A8]),
    },
    FlagPreset {
        name: "Trans",
        aliases: &["Transgender"],
        stripes: &hex_sequence([0x5BCEFA, 0xF5A9B8, 0xFFFFFF, 0xF5A9B8, 0x5BCEFA]),
    },
];


/// Convert a fixed-size array of u32s to colors, such that \[0xABCDEF, ...] => \[Color(0xAB, 0xCD, 0XEF), ...].
/// This const function allows for preset flags to be written easily without resorting to macros.
const fn hex_sequence<const N: usize>(hexes: [u32; N]) -> [Color; N] {
    let mut output = [Color::from_rgb(0, 0, 0); N];

    let mut i = 0;

    while i < N {
        let c = hexes[i];
        assert!(c < 256*256*256);

        let b = (c % 256) as u8;
        let g = ((c / 256) % 256) as u8;
        let r = ((c / 256) / 256) as u8;
        output[i] = Color::from_rgb(r, g, b);

        i += 1;
    }

    output
}


/// Iterate over all flag presets
pub fn iter_flag_presets() -> impl Iterator<Item=FlagPreset> {
    FLAG_PRESETS.into_iter().copied()
}

/// Find a flag preset by either its given name or any of its aliases
pub fn flag_by_name(name: &str) -> Option<FlagPreset> {
    iter_flag_presets()
        .find(|flag| {
            flag.name.eq_ignore_ascii_case(name)
                || flag.aliases.iter().any(|alias| alias.eq_ignore_ascii_case(name))
        })
}


/// Find the default flag preset
pub fn default_flag_preset() -> FlagPreset {
    flag_by_name("lesbian").expect("This is a built in flag")
}
