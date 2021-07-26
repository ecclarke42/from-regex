use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use quote::ToTokens;
use syn::spanned::Spanned;

mod captures;
mod impl_enum;
mod impl_struct;

// TODO: for unit structs/variants, don't require a named capture to
// capture an entire string?

// TODO: us from_str instead of .into in conversions?

/// # Derive FromRegex
///
/// ## Implementation Notes
///
/// - Default implementations of `from_regex` will only match if the *entire string* is matched
///
/// ## Usage with Structs
///
/// ### Item Level Attributes
///
///
///
/// ## Usage with Enums
///
/// ### Item Level Attributes
///
/// - Match Mode: TODO
///
///
#[proc_macro_error]
#[proc_macro_derive(FromRegex, attributes(from_regex))]
pub fn derive_regex(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    Item::from(&input).into_token_stream().into()
}

/// Generates the same code as [`FromRegex`], but also adds a `std::str::FromStr` implementation (with `()` error type)
#[proc_macro_error]
#[proc_macro_derive(FromStr, attributes(from_regex))]
pub fn derive_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let ident = input.ident.clone();
    let item = Item::from(&input);

    let stream = quote! {
        #item
        impl std::str::FromStr for #ident {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_regex(s).ok_or(())
            }
        }
    };

    stream.into()
}

const ATTRIBUTE: &str = "from_regex";

enum Item<'a> {
    Enum(impl_enum::Item<'a>),
    Struct(impl_struct::Item<'a>),
}

impl<'a> From<&'a syn::DeriveInput> for Item<'a> {
    fn from(input: &'a syn::DeriveInput) -> Self {
        let syn::DeriveInput {
            data, attrs, ident, ..
        } = input;
        match data {
            syn::Data::Enum(data_enum) => Item::Enum(impl_enum::Item::new(
                ident,
                &attrs,
                data_enum.variants.iter(),
            )),
            syn::Data::Struct(syn::DataStruct { fields, .. }) => {
                Item::Struct(impl_struct::Item::new(ident, &attrs, &fields))
            }
            syn::Data::Union(syn::DataUnion { union_token, .. }) => {
                abort!(union_token.span(), "Unsupported item type")
            }
        }
    }
}

impl<'a> ToTokens for Item<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Item::Enum(item) => item.to_tokens(tokens),
            Item::Struct(item) => item.to_tokens(tokens),
        }
    }
}

// Iterator representing crate specific attribtes
struct Attributes<'a> {
    iter: core::slice::Iter<'a, syn::Attribute>,
    list: Option<syn::punctuated::IntoIter<syn::NestedMeta>>,
}
impl<'a> From<&'a [syn::Attribute]> for Attributes<'a> {
    fn from(attrs: &'a [syn::Attribute]) -> Self {
        Self {
            iter: attrs.iter(),
            list: None,
        }
    }
}
impl<'a> Iterator for Attributes<'a> {
    type Item = syn::NestedMeta;
    fn next(&mut self) -> Option<Self::Item> {
        // Consume the existing list first
        if let Some(list) = &mut self.list {
            if let Some(nested) = list.next() {
                return Some(nested);
            }
        }

        loop {
            // Then return to the base iterator
            let next = self.iter.next()?;
            if let syn::Meta::List(list) = next.parse_meta().expect("failed to parse attr meta") {
                if list.path.is_ident(ATTRIBUTE) {
                    // If we have a list of attrs, return the first meta
                    // and hold on to the rest
                    let mut list = list.nested.into_iter();
                    if let Some(next) = list.next() {
                        self.list = Some(list);
                        return Some(next);
                    }
                }
            }
        }
    }
}
