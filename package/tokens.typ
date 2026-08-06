// The engine and the shared design tokens it publishes.
//
// The palette lives in Rust (`engine/src/html/tokens.rs`): the HTML stylesheet
// bakes it in, and `tokens()` hands the same values back here as TOML. Neither
// output owns a copy, so the callout colours and labels cannot drift apart.
//
// The plugin is instantiated here and imported everywhere else, so a compile
// loads `engine.wasm` and decodes the palette exactly once.

#let engine = plugin("engine.wasm")

// Light values only — Typst renders the paged output, which has no dark mode.
#let tokens = toml(engine.tokens())
