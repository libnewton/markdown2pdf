//! The shared design tokens — the one place a colour or a callout label is
//! written down.
//!
//! `css.rs` bakes these into the stylesheet's custom-property block, and
//! `tokens()` in `lib.rs` hands the same values to the Typst templates as TOML.
//! Neither renderer owns a copy, so the two outputs cannot drift apart the way
//! a hand-synced palette does.

use std::sync::OnceLock;

/// One callout kind: its accent, its background and its default label.
pub(crate) struct Admonition {
    pub kind: &'static str,
    /// `(light, dark)`.
    pub accent: (&'static str, &'static str),
    /// `(light, dark)`.
    pub bg: (&'static str, &'static str),
    pub en: &'static str,
    pub de: &'static str,
}

/// The callout palette. `info` is also the fallback for an unknown kind, so it
/// must stay in this list.
///
/// KNOWN LIMITATION: four light accents fall below WCAG AA (4.5:1) as label
/// text on their own tint — tip 2.60:1, warning 3.07:1, success 3.15:1,
/// danger 4.41:1. Both outputs share these values, so correcting them changes
/// the PDF; deliberately deferred. The upgrade path is a separate, darker
/// `-label` token for the text, leaving the accent for the border (non-text
/// contrast only needs 3:1).
pub(crate) const ADMONITIONS: &[Admonition] = &[
    Admonition {
        kind: "success",
        accent: ("#16a34a", "#4ade80"),
        bg: ("#f0fdf4", "#10241a"),
        en: "SUCCESS",
        de: "Erfolg",
    },
    Admonition {
        kind: "warning",
        accent: ("#d97706", "#f5b544"),
        bg: ("#fffbeb", "#2a1f08"),
        en: "WARNING",
        de: "Warnung",
    },
    Admonition {
        kind: "tip",
        accent: ("#0ea5e9", "#56c6f5"),
        bg: ("#f0f9ff", "#0b2130"),
        en: "TIP",
        de: "Tipp",
    },
    Admonition {
        kind: "info",
        accent: ("#2563eb", "#7aa6ff"),
        bg: ("#eff6ff", "#111e35"),
        en: "INFO",
        de: "Info",
    },
    Admonition {
        kind: "danger",
        accent: ("#dc2626", "#ff8080"),
        bg: ("#fef2f2", "#2b1414"),
        en: "DANGER",
        de: "Gefahr",
    },
    Admonition {
        kind: "note",
        accent: ("#6b7280", "#a3adba"),
        bg: ("#f9fafb", "#1b1f26"),
        en: "NOTE",
        de: "Hinweis",
    },
    Admonition {
        kind: "caution",
        accent: ("#b91c1c", "#ff8f8f"),
        bg: ("#fef2f2", "#2b1414"),
        en: "CAUTION",
        de: "Vorsicht",
    },
    Admonition {
        kind: "important",
        accent: ("#7e22ce", "#cd96f7"),
        bg: ("#faf5ff", "#21132f"),
        en: "IMPORTANT",
        de: "Wichtig",
    },
];

/// `(name, light, dark)` -> `--md-<name>: light-dark(<light>, <dark>)`.
///
/// The `t-*` entries are the syntax-highlighting classes in `highlight.rs`;
/// they are checked against `surface` by `token_colours_meet_wcag_aa`.
pub(crate) const BASE: &[(&str, &str, &str)] = &[
    ("bg", "#ffffff", "#14171c"),
    ("fg", "#16191d", "#dde2e9"),
    ("heading", "#333333", "#eef1f5"),
    ("muted", "#6b7280", "#9aa4b2"),
    ("accent", "#0074de", "#62b0ff"),
    ("rule", "#e3e6ea", "#2b313a"),
    ("surface", "#f6f7f9", "#1b1f26"),
    ("surface-2", "#eef0f3", "#232830"),
    ("mark-bg", "#fef08a", "#6b5a12"),
    ("mark-fg", "#453c05", "#fdf6d8"),
    ("quote-bg", "#f8f9fa", "#1a1e25"),
    ("scrim", "rgba(16, 24, 40, .3)", "rgba(0, 0, 0, .55)"),
    ("shadow-color", "rgba(16, 24, 40, .12)", "rgba(0, 0, 0, .55)"),
    ("t-c", "#666f7b", "#7f8a9c"),
    ("t-s", "#0a7d55", "#6dd3a4"),
    ("t-n", "#b45309", "#f0b464"),
    ("t-k", "#9333ea", "#c79bf5"),
    ("t-t", "#0369a1", "#6cc4f0"),
    ("t-m", "#be185d", "#f58bb4"),
    ("t-f", "#1d4ed8", "#93b4fb"),
    ("t-o", "#4b5563", "#a8b3c2"),
    ("t-p", "#0f766e", "#5ecfc0"),
    ("t-v", "#b91c1c", "#f2918c"),
    ("ok", "#16a34a", "#4ade80"),
];

/// `(name, value)` -> `--md-<name>: <value>` — no light/dark pair. Emitted
/// after `BASE` because `shadow` refers back to `--md-shadow-color`.
pub(crate) const PLAIN: &[(&str, &str)] = &[
    (
        "shadow",
        "0 1px 2px var(--md-shadow-color), 0 8px 24px var(--md-shadow-color)",
    ),
    ("measure", "56rem"),
];

/// The custom-property declarations, without the surrounding selector. Built
/// once per plugin instance so a render never pays for it.
pub(crate) fn declarations() -> &'static str {
    static BLOCK: OnceLock<String> = OnceLock::new();
    BLOCK.get_or_init(|| {
        let mut out = String::with_capacity(2048);
        for (name, light, dark) in BASE {
            out.push_str(&format!("  --md-{name}: light-dark({light}, {dark});\n"));
        }
        for (name, value) in PLAIN {
            out.push_str(&format!("  --md-{name}: {value};\n"));
        }
        for a in ADMONITIONS {
            out.push_str(&format!(
                "  --md-adm-{}: light-dark({}, {});\n",
                a.kind, a.accent.0, a.accent.1
            ));
        }
        for a in ADMONITIONS {
            out.push_str(&format!(
                "  --md-adm-{}-bg: light-dark({}, {});\n",
                a.kind, a.bg.0, a.bg.1
            ));
        }
        out
    })
}

/// The default label for a callout kind. An unrecognised kind falls back to
/// `info`, matching `admonitions.typ`.
pub(crate) fn label(kind: &str, german: bool) -> &'static str {
    let a = ADMONITIONS
        .iter()
        .find(|a| a.kind == kind)
        .or_else(|| ADMONITIONS.iter().find(|a| a.kind == "info"))
        .expect("info is always present");
    if german {
        a.de
    } else {
        a.en
    }
}

/// Every token the Typst templates need, as TOML.
///
/// Only the light values are published: Typst renders the paged output, which
/// has no dark mode. The dark halves exist solely for the stylesheet.
pub(crate) fn as_toml() -> String {
    let mut out = String::with_capacity(1024);
    out.push_str("[base]\n");
    for (name, light, _) in BASE {
        out.push_str(&format!("\"{name}\" = \"{light}\"\n"));
    }
    for a in ADMONITIONS {
        out.push_str(&format!(
            "\n[admonition.{}]\naccent = \"{}\"\nbg = \"{}\"\nen = \"{}\"\nde = \"{}\"\n",
            a.kind, a.accent.0, a.bg.0, a.en, a.de
        ));
    }
    out
}
