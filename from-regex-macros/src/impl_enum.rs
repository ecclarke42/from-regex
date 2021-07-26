use std::collections::HashMap;

use heck::{ShoutySnekCase, SnekCase};
use proc_macro_error::abort;
use quote::quote;
use syn::spanned::Spanned;

use crate::captures;

// TODO: make sure variants match full text

pub struct Item<'a> {
    ident: &'a syn::Ident,
    attrs: ItemAttributes,
    variants: Vec<Variant<'a>>,
}

pub struct ItemAttributes {
    match_mode: MatchMode,
}
// TODO: document match mode... First generates multiple regex consts,
// longest only generates a master regex for the whole enum
enum MatchMode {
    First,
    Longest,
}

// Regex default or (a)|(b) is to match the longest variant
impl Default for MatchMode {
    fn default() -> Self {
        Self::Longest
    }
}

const ENUM_ATTRIBUTE_MATCH_MODE: &str = "match_mode";
const ENUM_ATTRIBUTE_MATCH_MODE_LONGEST: &str = "longest";
const ENUM_ATTRIBUTE_MATCH_MODE_FIRST: &str = "first";

impl From<&[syn::Attribute]> for ItemAttributes {
    fn from(attrs: &[syn::Attribute]) -> Self {
        let mut match_mode = MatchMode::Longest;
        for meta in crate::Attributes::from(attrs) {
            if let syn::NestedMeta::Meta(syn::Meta::NameValue(syn::MetaNameValue {
                path,
                lit: syn::Lit::Str(lit),
                ..
            })) = meta
            {
                if path.is_ident(ENUM_ATTRIBUTE_MATCH_MODE) {
                    match lit.value().as_str() {
                        ENUM_ATTRIBUTE_MATCH_MODE_LONGEST => match_mode = MatchMode::Longest,
                        ENUM_ATTRIBUTE_MATCH_MODE_FIRST => match_mode = MatchMode::First,
                        other => abort!(lit.span(), "Unknown match mode: {}", other),
                    }
                }
            }
        }

        Self { match_mode }
    }
}

impl<'a> quote::ToTokens for Item<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self.attrs.match_mode {
            MatchMode::Longest => self.to_tokens_longest(),
            MatchMode::First => self.to_tokens_first(),
        })
    }
}

impl<'a> Item<'a> {
    pub fn new<V: Iterator<Item = &'a syn::Variant>>(
        ident: &'a syn::Ident,
        attrs: &'a [syn::Attribute],
        variants: V,
    ) -> Self {
        Self {
            ident,
            attrs: attrs.into(),
            variants: variants.map(Variant::new).collect(),
        }
    }

    fn name_shouty(&self) -> String {
        self.ident.to_string().TO_SHOUTY_SNEK_CASE()
    }

    /// Get the constructor for the default variant (if it exists)
    fn default_constructor(&self) -> Option<proc_macro2::TokenStream> {
        let mut default = None;
        for variant in self.variants.iter() {
            if variant.attrs.default {
                if let Some((existing, _)) = default {
                    abort!(variant.ident.span(), "More than one default varaiant. {} identified as default, but {} is already set", variant.ident, existing);
                }

                let ident = self.ident;
                let variant_ident = variant.ident;

                default = Some((
                    variant_ident,
                    match &variant.fields {
                        syn::Fields::Named(syn::FieldsNamed { named, .. }) => {
                            let fields = named
                                .iter()
                                .map(|syn::Field { ident, ty, .. }| {
                                    quote! { #ident: <#ty>::default() }
                                })
                                .collect::<Vec<_>>();
                            quote! { Some(#ident::#variant_ident { #( #fields, )* }) }
                        }

                        syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => {
                            let fields = unnamed
                                .iter()
                                .map(|syn::Field { ty, .. }| {
                                    quote! { <#ty>::default() }
                                })
                                .collect::<Vec<_>>();

                            quote! { Some(#ident::#variant_ident ( #( #fields, )* )) }
                        }

                        syn::Fields::Unit => quote! { Some(#ident::#variant_ident) },
                    },
                ))
            }
        }
        default.map(|(_, stream)| stream)
    }

    /// [`ToTokens`] for [`MatchMode::Longest`] with combined regex
    /// (no transparent items)
    ///
    /// TODO: combine below to generalized func
    ///
    /// Generate a definition statement for the combined regex.
    /// Returns a mapping of variant names to capture groups (with updated
    /// captures) and the token stream defining the regex constant.
    ///
    /// Capture groups in each variant will be prepended with the variant and
    /// wrapped with a varaiant-specific capture group.
    ///
    /// For example:
    /// ```ignore
    /// enum Test {
    ///     #[from_regex(pattern = "(?P<one>[0-9]{5})-(?P<two>[a-z]*)?")]
    ///     VariantA {
    ///         one: String,
    ///         two: Option<String>,
    ///     }
    ///     #[from_regex(pattern = "some(?P<0>thing)")]
    ///     VariantB(String),
    ///     #[from_regex(default)]
    ///     C,
    /// }
    /// ```
    /// Generates
    /// ```ignore
    /// lazy_static! {
    ///     static ref TEST_REGEX: Regex = Regex::new("(?P<VariantA>(?P<VariantA_one>[0-9]{5})-(?P<VariantA_two>[a-z]*)?)|(?P<VariantB>some(?P<VariantB_0>thing))")
    /// }
    /// ```
    fn to_tokens_longest(&self) -> proc_macro2::TokenStream {
        // Combine regex patterns we have into one single pattern
        let ident = self.ident;
        let regex_ident = syn::Ident::new(&format!("{}_REGEX", self.name_shouty()), ident.span());

        let mut patterns = Vec::new();
        let mut from_capture_impls = Vec::new();
        let mut from_regex_impls = Vec::new();

        let mut match_locations_patterened = Vec::new();
        let mut match_locations_transparent = Vec::new();

        for variant in self.variants.iter() {
            let ident = variant.ident;
            let ident_str = ident.to_string();

            // If a patterned variant, collect it's
            match &variant.attrs.pattern {
                VariantPattern::Some(pattern_lit) => {
                    let mut pattern = pattern_lit.value();

                    // Get group / field pairs
                    let groups = captures::from_regex_pattern(&pattern.clone())
                        .into_iter()
                        .map(|(group, optional)| {
                            // Prepend group name with variant name
                            let new_group = if group.starts_with('_') {
                                format!("{}{}", ident_str, group)
                            } else {
                                format!("{}_{}", ident_str, group)
                            };

                            pattern = pattern
                                .replace(&format!("?P<{}>", group), &format!("?P<{}>", new_group));

                            (new_group, optional)
                        })
                        .collect::<HashMap<_, _>>();

                    // For borrow
                    let captured_groups = groups.iter().map(|(s, b)| (s.as_str(), *b)).collect();

                    // Collect variant patterns
                    let ident_str_lit = syn::LitStr::new(&ident_str, ident.span());
                    patterns.push(format!("(?P<{}>{})", ident_str, pattern));

                    // Generate a variant specific `__from_regex_capture_x` (will unwrap unless transparent)
                    let (from_capture_fn, from_capture_impl) =
                        variant.impl_from_capture(&captured_groups, true);

                    from_capture_impls.push(from_capture_impl);
                    from_regex_impls.push(quote! {
                        if let Some(cap) = &captures {
                            if cap.name(#ident_str_lit).is_some() {
                                if let Some(value) = Self::#from_capture_fn(&cap) {
                                    return Some(value);
                                }
                            }
                        }
                    });

                    match_locations_patterened.push(quote! {
                        if cap.name(#ident_str_lit).is_some() {
                            if let Some(value) = Self::#from_capture_fn(&cap) {
                                ranges.insert_if_empty(range, value);
                                continue;
                            }
                        }
                    });
                }

                VariantPattern::Transparent => {
                    let inner = variant.transparent_inner_type().unwrap();
                    from_regex_impls.push(quote! {
                        if let Some(inner) = <#inner>::from_regex(s) {
                            return Some(Self::#ident(inner));
                        }
                    });

                    // If we have transparent items to search for, use their search
                    // functions to generate sibling `RangeMap`s and use our extension
                    // trait to keep segments longer than already found
                    match_locations_transparent.push(quote! {
                        ranges.merge_only_longest(
                            <#inner>::match_locations(s).into_iter().map(|(r, v)| {
                                (r, Self::#ident(v))
                            })
                        );
                    });
                }

                VariantPattern::None => { /* No Op, since we'll never return these from regex */ }
            }
        }
        let combined_pattern = patterns.join("|");

        // Default return for from_regex
        let return_from_regex = self
            .default_constructor()
            .unwrap_or_else(|| quote! { None });

        quote! {
            from_regex::lazy_static! {
                static ref #regex_ident: from_regex::Regex = from_regex::Regex::new(#combined_pattern).expect("Failed to compile regex");
            }
            impl #ident {
                #(
                    #from_capture_impls
                )*
            }
            impl from_regex::FromRegex for #ident {
                fn from_regex(s: &str) -> Option<Self> {
                    let captures = #regex_ident.captures(s).filter(|cap| cap[0].len() == s.len());
                    #(
                        #from_regex_impls
                    )*
                    #return_from_regex
                }

                fn match_locations(s: &str) -> from_regex::RangeMap<usize, Self> {
                    use from_regex::TextMap;
                    let mut ranges = from_regex::RangeMap::new();
                    for cap in #regex_ident.captures_iter(s) {
                        let range = cap.get(0).unwrap().range();
                        #(
                            #match_locations_patterened
                        )*
                    }

                    #(
                        #match_locations_transparent
                    )*

                    ranges
                }
            }
        }
    }

    /// [`ToTokens`] for [`MatchMode::First`] (always separated regex)
    ///
    ///
    fn to_tokens_first(&self) -> proc_macro2::TokenStream {
        let ident = self.ident;

        let enum_name_shouty = self.name_shouty();

        let mut regex_defs = Vec::new();
        let mut from_capture_impls = Vec::new();
        let mut from_regex_impls = Vec::new();
        let mut match_locations_impls = Vec::new();

        for variant in self.variants.iter() {
            let ident = variant.ident;

            // If a patterned variant, collect it's
            match &variant.attrs.pattern {
                VariantPattern::Some(pattern_lit) => {
                    let pattern = pattern_lit.value();

                    let regex_ident = syn::Ident::new(
                        &format!(
                            "{}_{}_REGEX",
                            enum_name_shouty,
                            variant.ident.to_string().TO_SHOUTY_SNEK_CASE()
                        ),
                        self.ident.span(),
                    );

                    // Generate a regex constant for this variant only
                    regex_defs.push(quote! {
                        static ref #regex_ident: from_regex::Regex = from_regex::Regex::new(#pattern).expect("Failed to compile regex");
                    });

                    // Generate a variant specific `__from_regex_capture_x`
                    // (will unwrap unless transparent)
                    let (from_capture_fn, from_capture_impl) =
                        variant.impl_from_capture(&captures::from_regex_pattern(&pattern), false);
                    from_capture_impls.push(from_capture_impl);

                    // Add a section for `from_regex` calling this variant's
                    // conversion method
                    from_regex_impls.push(quote! {                    
                        if let Some(captures) = #regex_ident.captures(s).filter(|cap| cap[0].len() == s.len()) {
                            if let Some(value) = Self::#from_capture_fn(&captures) {
                                return Some(value);
                            }
                        }
                    });

                    match_locations_impls.push(quote! {
                        for cap in #regex_ident.captures_iter(s) {
                            if let Some(value) = Self::#from_capture_fn(&cap) {
                                ranges.insert_if_empty(cap.get(0).unwrap().range(), value);
                            }
                        }
                    });
                }

                VariantPattern::Transparent => {
                    let inner = variant.transparent_inner_type().unwrap();
                    from_regex_impls.push(quote! {
                        if let Some(inner) = <#inner>::from_regex(s) {
                            return Some(Self::#ident(inner));
                        }
                    });

                    match_locations_impls.push(quote! {
                        for (range, value) in <#inner>::match_locations(s) {
                            ranges.insert_if_empty(range, Self::#ident(value));
                        }
                    });
                }

                VariantPattern::None => { /* No Op, since we'll never return these from regex */ }
            }
        }

        // Default return for from_regex
        let return_from_regex = self
            .default_constructor()
            .unwrap_or_else(|| quote! { None });

        quote! {
            from_regex::lazy_static! {
                #(
                    #regex_defs
                )*
            }
            impl #ident {
                #(
                    #from_capture_impls
                )*
            }
            impl from_regex::FromRegex for #ident {
                fn from_regex(s: &str) -> Option<Self> {
                    #(
                        #from_regex_impls
                    )*
                    #return_from_regex
                }

                fn match_locations(s: &str) -> from_regex::RangeMap<usize, Self> {
                    use from_regex::TextMap;
                    let mut ranges = from_regex::RangeMap::new();

                    #(
                        #match_locations_impls
                    )*

                    ranges
                }
            }
        }
    }
}

pub struct Variant<'a> {
    ident: &'a syn::Ident,
    attrs: VariantAttributes,
    fields: &'a syn::Fields,
}

pub struct VariantAttributes {
    pattern: VariantPattern,
    default: bool,
}
impl VariantAttributes {
    fn is_transparent(&self) -> bool {
        matches!(self.pattern, VariantPattern::Transparent)
    }
}

enum VariantPattern {
    None,
    Some(syn::LitStr),
    Transparent,
}

const VARIANT_ATTRIBUTE_PATTERN: &str = "pattern";
const VARIANT_ATTRIBUTE_DEFAULT: &str = "default";
const VARIANT_ATTRIBUTE_TRANSPARENT: &str = "transparent";

impl<'a> From<&'a [syn::Attribute]> for VariantAttributes {
    fn from(attrs: &'a [syn::Attribute]) -> Self {
        let mut pattern = VariantPattern::None;
        let mut default = false;
        for attr in attrs {
            if let syn::Meta::List(list) = attr.parse_meta().expect("failed to parse attr meta") {
                if list.path.is_ident(crate::ATTRIBUTE) {
                    let attr_span = list.span();
                    for nested in list.nested {
                        if let syn::NestedMeta::Meta(meta) = nested {
                            match meta {
                                syn::Meta::NameValue(syn::MetaNameValue {
                                    path,
                                    lit: syn::Lit::Str(lit),
                                    ..
                                }) => {
                                    if path.is_ident(VARIANT_ATTRIBUTE_PATTERN) {
                                        match pattern {
                                            VariantPattern::None => pattern = VariantPattern::Some(lit),
                                            VariantPattern::Some(_) => abort!(attr_span, "Pattern already defined on this variant"),
                                            VariantPattern::Transparent => abort!(attr_span, "Variants can only have a pattern or be transparent (not both)"),
                                        }
                                    }
                                }
                                syn::Meta::Path(path) => {
                                    if path.is_ident(VARIANT_ATTRIBUTE_DEFAULT) {
                                        default = true;
                                    } else if path.is_ident(VARIANT_ATTRIBUTE_TRANSPARENT) {
                                        match pattern {
                                            VariantPattern::None => pattern = VariantPattern::Transparent,
                                            VariantPattern::Some(_) => abort!(attr_span, "Pattern already defined on this variant"),
                                            VariantPattern::Transparent => abort!(attr_span, "Variants can only have a pattern or be transparent (not both)"),
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Self { pattern, default }
    }
}

impl<'a> Variant<'a> {
    pub fn new(variant: &'a syn::Variant) -> Self {
        let ident = &variant.ident;
        let attrs = VariantAttributes::from(variant.attrs.as_ref());
        let fields = &variant.fields;
        if attrs.is_transparent()
            && !matches!(fields, syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, ..}) if unnamed.len() == 1)
        {
            abort!(
                ident.span(),
                "The `transparent` attribute is only available for single element tuple structs"
            );
        }
        Self {
            ident,
            attrs,
            fields,
        }
    }

    fn from_capture_fn_ident(&self) -> syn::Ident {
        syn::Ident::new(
            &format!(
                "__from_regex_capture_{}",
                self.ident.to_string().to_snek_case()
            ),
            self.ident.span(),
        )
    }

    pub fn impl_from_capture(
        &self,
        captured_groups: &crate::captures::Groups,
        prefixed: bool,
    ) -> (syn::Ident, proc_macro2::TokenStream) {
        let variant = self.ident;
        let prefix = if prefixed {
            Some(variant.to_string())
        } else {
            None
        };
        let fn_ident = self.from_capture_fn_ident();

        let case_attr = if prefixed {
            quote! {
                #[allow(non_snake_case)]
            }
        } else {
            quote! {}
        };

        // TODO: move self parts into method so we don't need to match on fields?
        let implementation = match self.fields {
            syn::Fields::Named(syn::FieldsNamed { .. }) => {
                let (field_names, field_statements) = crate::captures::impl_fields_from_capture(
                    captured_groups,
                    &self.fields,
                    prefix.as_deref(),
                );

                quote! {
                    #case_attr
                    fn #fn_ident(captures: &from_regex::Captures) -> Option<Self> {
                        #(#field_statements)*
                        Some(Self::#variant { #(#field_names),* })
                    }
                }
            }
            syn::Fields::Unnamed(syn::FieldsUnnamed { .. }) => {
                let (assigned_names, field_statements) = crate::captures::impl_fields_from_capture(
                    captured_groups,
                    &self.fields,
                    prefix.as_deref(),
                );

                quote! {
                    #case_attr
                    fn #fn_ident(captures: &from_regex::Captures) -> Option<Self> {
                        #(#field_statements)*
                        Some(Self::#variant ( #(#assigned_names),* ))
                    }
                }
            }
            syn::Fields::Unit => {
                quote! {
                    fn #fn_ident(captures: &from_regex::Captures) -> Option<Self> {
                        Some(Self::#variant)
                    }
                }
            }
        };

        (fn_ident, implementation)
    }

    fn transparent_inner_type(&self) -> Option<&syn::Type> {
        if let syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) = self.fields {
            unnamed.first().map(|f| &f.ty)
        } else {
            None
        }
    }
}
