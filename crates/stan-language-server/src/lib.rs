//! Protocol-independent document and diagnostic infrastructure for the Stan LSP.
//!
//! This crate owns open-document state, UTF-16 position mapping, LSP value
//! adaptation, workspace includes, and stanc3 process integration. Stan syntax
//! and semantic rules remain in `stan-language`.

#![deny(missing_docs)]

/// Shared-diagnostic to LSP conversion and merging.
pub mod diagnostics;
/// Versioned document state and cached analysis.
pub mod document;
/// Protocol-independent implementations of advertised editor features.
pub mod features;
/// Low-level line start and encoding conversion.
pub mod line_index;
/// Authoritative language-range to LSP-range mapping.
pub mod mapper;
/// Asynchronous external compiler integration.
pub mod stanc;
/// Folder indexing, configuration, overlays, and include resolution.
pub mod workspace;
