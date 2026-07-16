# Helix patch inventory

## `0001-add-lantern-range-navigation.patch`

- **Upstream:** Helix `14d6bc0febed9c692048271a8ae2362ac969c6e0`
- **Purpose:** Select an exact evidence range, export the primary selection,
  and resolve one definition plus at most eight references through Helix's
  active LSP session.
- **Boundary:** Three command handlers and registrations in `helix-term`;
  selection input is bounded to 64 KiB, LSP targets are repository-local files
  no larger than 512 KiB, and output uses only the session-scoped bridge path.
- **Validation:** Helix formatting and `cargo check -p helix-term`, plus the
  live Lantern selection probe.
- **Removal condition:** Replace the command when Helix exposes a stable typed
  external navigation boundary, or reverse the frontend decision through an
  explicit architecture decision record.

The patch does not add model, daemon, Git, network, or policy behavior to
Helix. It converts a validated range into a native selection and serializes
selection or symbol context without putting source text in shell arguments.
Unavailable LSP support and unresolved definitions are explicit errors; the
command does not substitute literal search.

## `0002-add-picker-mouse-interaction.patch`

- **Upstream:** Helix `14d6bc0febed9c692048271a8ae2362ac969c6e0`
- **Purpose:** Make the generic picker mouse-aware: wheel and click navigation
  in the result list, drag selection in document previews, and promotion of a
  preview selection into the normal editor selection when opening normally,
  in a split, or through `Ctrl-a` and its configured editor binding.
- **Boundary:** One `helix-term` picker implementation. No Lantern protocol,
  process, model, Git, or filesystem bridge behavior is added.
- **Validation:** Helix formatting, `cargo check -p helix-term`, all 15
  `helix-term` library tests, targeted Clippy, and a live picker drag proving an
  exact non-empty `AGENTS.md:5:5-5:26` selection reaches the existing export
  command.
- **Removal condition:** Upstream equivalent picker mouse support, or an
  explicit architecture decision that reverses the Helix frontend choice.

The preview selection is deliberately temporary. Changing the highlighted
picker row or query clears it. Promotion opens the selected item through the
picker's existing callback and then installs the exact range in the resulting
editor document, keeping selection ownership in Helix.
