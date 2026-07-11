//! Import planning for generated Rust code.
//!
//! [`ImportItem`] describes one `use` item, and [`ImportSet`] collects,
//! deduplicates, groups, and renders those items as deterministic `use`
//! statements.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// The alias applied to an imported item.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Alias {
    /// Wildcard discard: `use X as _`.
    Anonymous,
    /// Named rename: `use X as Foo`.
    Rename(&'static str),
}

/// A single item to be imported into generated code.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportItem {
    /// Full path to the imported item, e.g. `"gpui::Render"`.
    pub path: &'static str,
    /// Optional alias applied to the import.
    pub alias: Option<Alias>,
}

impl ImportItem {
    /// An import with no alias.
    pub const fn path(path: &'static str) -> Self {
        Self { path, alias: None }
    }

    /// An import with an alias.
    pub const fn aliased(path: &'static str, alias: Alias) -> Self {
        Self {
            path,
            alias: Some(alias),
        }
    }
}

/// A deduplicated, ordered collection of [`ImportItem`]s.
#[derive(Default)]
pub struct ImportSet(std::collections::BTreeSet<ImportItem>);

impl ImportSet {
    /// Insert a single item.
    pub fn add(&mut self, item: ImportItem) {
        self.0.insert(item);
    }

    /// Insert a slice of items.
    pub fn extend_items(&mut self, items: &[ImportItem]) {
        self.0.extend(items.iter().cloned());
    }

    /// Insert an iterator of items.
    pub fn extend(&mut self, items: impl IntoIterator<Item = ImportItem>) {
        self.0.extend(items);
    }

    /// Render all imports as grouped `use parent::{a, b as c};` token streams.
    ///
    /// This is an infallible wrapper for generated-code call sites that use
    /// static, known-good paths. Use [`Self::try_to_token_stream`] when import
    /// paths may come from user input.
    pub fn to_token_stream(&self) -> TokenStream {
        self.try_to_token_stream()
            .expect("valid import parent path in ImportSet")
    }

    /// Fallible version of [`Self::to_token_stream`].
    pub fn try_to_token_stream(&self) -> syn::Result<TokenStream> {
        let mut grouped: BTreeMap<String, Vec<(&'static str, Option<&Alias>)>> = BTreeMap::new();

        for item in &self.0 {
            let (parent, name) = item.path.rsplit_once("::").unwrap_or(("", item.path));
            grouped
                .entry(parent.to_string())
                .or_default()
                .push((name, item.alias.as_ref()));
        }

        let mut tokens = TokenStream::new();
        for (parent, items) in &grouped {
            if parent.is_empty() {
                continue;
            }

            let parent_path: syn::Path = syn::parse_str(parent).map_err(|err| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("invalid import parent path `{parent}`: {err}"),
                )
            })?;

            let item_tokens: Vec<TokenStream> = items
                .iter()
                .map(|(name, alias)| {
                    let name_ident = format_ident!("{}", name);
                    match alias {
                        Some(Alias::Anonymous) => quote! { #name_ident as _ },
                        Some(Alias::Rename(alias)) => {
                            let alias_ident = format_ident!("{}", alias);
                            quote! { #name_ident as #alias_ident }
                        },
                        None => quote! { #name_ident },
                    }
                })
                .collect();

            if item_tokens.len() == 1 {
                let single = &item_tokens[0];
                tokens.extend(quote! { use #parent_path::#single; });
            } else {
                tokens.extend(quote! { use #parent_path::{#(#item_tokens),*}; });
            }
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact(tokens: TokenStream) -> String {
        tokens
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect()
    }

    #[test]
    fn deduplicates_imports() {
        let mut imports = ImportSet::default();
        imports.add(ImportItem::path("gpui::Render"));
        imports.add(ImportItem::path("gpui::Render"));

        assert_eq!(compact(imports.to_token_stream()), "usegpui::Render;");
    }

    #[test]
    fn groups_deterministically_by_parent_module() {
        let mut imports = ImportSet::default();
        imports.add(ImportItem::path("gpui_component::table::DataTable"));
        imports.add(ImportItem::path("gpui::Window"));
        imports.add(ImportItem::path("gpui::App"));
        imports.add(ImportItem::path("gpui_component::table::TableState"));

        assert_eq!(
            compact(imports.to_token_stream()),
            "usegpui::{App,Window};usegpui_component::table::{DataTable,TableState};"
        );
    }

    #[test]
    fn renders_anonymous_and_rename_aliases() {
        let mut imports = ImportSet::default();
        imports.add(ImportItem::aliased("gpui::ParentElement", Alias::Anonymous));
        imports.add(ImportItem::aliased(
            "gpui::AppContext",
            Alias::Rename("GpuiAppContext"),
        ));

        assert_eq!(
            compact(imports.to_token_stream()),
            "usegpui::{AppContextasGpuiAppContext,ParentElementas_};"
        );
    }

    #[test]
    fn skips_bare_imports() {
        let mut imports = ImportSet::default();
        imports.add(ImportItem::path("AlreadyInScope"));
        imports.add(ImportItem::path("gpui::Render"));

        assert_eq!(compact(imports.to_token_stream()), "usegpui::Render;");
    }

    #[test]
    fn reports_invalid_parent_path() {
        let mut imports = ImportSet::default();
        imports.add(ImportItem::path("gpui::123::Render"));

        let error = imports
            .try_to_token_stream()
            .expect_err("invalid import parent should fail");

        assert!(
            error
                .to_string()
                .contains("invalid import parent path `gpui::123`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn extends_imports_from_slices_and_iterators() {
        let mut imports = ImportSet::default();
        imports.extend_items(&[
            ImportItem::path("gpui::App"),
            ImportItem::path("gpui::Window"),
        ]);
        imports.extend([ImportItem::path("gpui::Context")]);

        assert_eq!(
            compact(imports.to_token_stream()),
            "usegpui::{App,Context,Window};"
        );
    }
}
