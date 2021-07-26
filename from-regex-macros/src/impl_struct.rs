use heck::ShoutySnekCase;
use quote::quote;

pub struct Item<'a> {
    ident: &'a syn::Ident,
    attrs: ItemAttributes,
    fields: &'a syn::Fields,
}

pub struct ItemAttributes {
    pattern: syn::LitStr,
}

const ITEM_ATTRIBUTE_PATTERN: &str = "pattern";

impl<'a> From<&'a [syn::Attribute]> for ItemAttributes {
    fn from(attrs: &'a [syn::Attribute]) -> Self {
        let mut pattern = None;

        for meta in crate::Attributes::from(attrs) {
            if let syn::NestedMeta::Meta(syn::Meta::NameValue(syn::MetaNameValue {
                path,
                lit: syn::Lit::Str(lit),
                ..
            })) = meta
            {
                if path.is_ident(ITEM_ATTRIBUTE_PATTERN) {
                    pattern = Some(lit);
                }
            }
        }

        let pattern = pattern.expect("Regex pattern must be present");

        Self { pattern }
    }
}

impl<'a> Item<'a> {
    pub fn new(
        ident: &'a syn::Ident,
        attrs: &'a [syn::Attribute],
        fields: &'a syn::Fields,
    ) -> Self {
        Self {
            ident,
            attrs: attrs.into(),
            fields,
        }
    }
}

impl<'a> quote::ToTokens for Item<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = self.ident;
        let pattern = self.attrs.pattern.value();
        let regex_const = syn::Ident::new(
            &format!("{}_REGEX", self.ident.to_string().TO_SHOUTY_SNEK_CASE()),
            self.ident.span(),
        );

        // This method is not necessary for the trait implementation,
        // but deduplicates some field-based logic for the others
        let from_capture_impl = match self.fields {
            syn::Fields::Named(syn::FieldsNamed { .. }) => {
                let pattern = self.attrs.pattern.value();
                let (field_names, field_statements) = crate::captures::impl_fields_from_capture(
                    &crate::captures::from_regex_pattern(&pattern),
                    &self.fields,
                    None,
                );

                quote! {
                    fn __from_regex_capture(captures: from_regex::Captures) -> Option<Self> {
                        #(#field_statements)*
                        Some(Self{ #(#field_names),* })
                    }
                }
            }
            syn::Fields::Unnamed(syn::FieldsUnnamed { .. }) => {
                let pattern = self.attrs.pattern.value();
                let (assigned_names, field_statements) = crate::captures::impl_fields_from_capture(
                    &crate::captures::from_regex_pattern(&pattern),
                    &self.fields,
                    None,
                );

                quote! {
                    fn __from_regex_capture(captures: from_regex::Captures) -> Option<Self> {
                        #(#field_statements)*
                        Some(Self( #(#assigned_names),* ))
                    }
                }
            }
            syn::Fields::Unit => {
                quote! {
                    fn __from_regex_capture(captures: from_regex::Captures) -> Option<Self> {
                        Some(Self)
                    }
                }
            }
        };

        // If unit struct, we don't need to capture (but we'll still find a
        // Match), since Regex::is_match doesn't ensure the entire string is
        // matched, just that a match exists in it.
        let impl_from_regex = if matches!(self.fields, syn::Fields::Unit) {
            quote! {
                fn from_regex(s: &str) -> Option<Self> {
                    match #regex_const.find(s) {
                        Some(mat) if (mat.end() - mat.start()) == s.len() => Some(Self),
                        Some(_) => None,
                        None => None,
                    }
                }
            }
        } else {
            quote! {
                fn from_regex(s: &str) -> Option<Self> {
                    match #regex_const.captures(s) {
                        Some(cap) if cap[0].len() == s.len() => Self::__from_regex_capture(cap),
                        Some(_) => None,
                        None => None,
                    }
                }
            }
        };

        // Similar to above, Unit struct doesn't need captures
        let impl_match_locations = if matches!(self.fields, syn::Fields::Unit) {
            quote! {
                fn match_locations(s: &str) -> from_regex::RangeMap<usize, Self> {
                    #regex_const.find_iter(s).map(|mat| (mat.range(), Self)).collect()
                }
            }
        } else {
            quote! {
                fn match_locations(s: &str) -> from_regex::RangeMap<usize, Self> {
                    #regex_const
                        .captures_iter(s)
                        .filter_map(|cap| {
                            // Unwrap is fine for get(0), because otherwise it wouldn't have matched
                            let range = cap.get(0).unwrap().range();
                            Self::__from_regex_capture(cap).map(|value| {
                                (range, value)
                            })
                        })
                        .collect()
                }
            }
        };

        tokens.extend(quote! {
            from_regex::lazy_static! {
                static ref #regex_const: from_regex::Regex = from_regex::Regex::new(#pattern).expect("Failed to compile regex");
            }
            impl #ident {
                #from_capture_impl
            }
            impl from_regex::FromRegex for #ident {
                #impl_from_regex
                #impl_match_locations
            }
        });
    }
}
