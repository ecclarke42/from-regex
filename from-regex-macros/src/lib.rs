use quote::quote;

mod captures;
mod impl_enum;
mod impl_struct;

/// # Derive FromRegex
///
/// ## Usage with Structs
///
///
///
/// ## Usage with Enums
///
#[proc_macro_derive(FromRegex, attributes(from_regex))]
pub fn derive_regex(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    inner_impl_from_regex(&input, false).into()
}

/// Generates the same code as [`FromRegex`], but also adds a `std::str::FromStr` implementation (with `()` error type)
#[proc_macro_derive(FromStr, attributes(from_regex))]
pub fn derive_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    inner_impl_from_regex(&input, true).into()
}

const ATTRIBUTE: &str = "from_regex";

fn inner_impl_from_regex(
    input: &syn::DeriveInput,
    impl_from_str: bool,
) -> proc_macro2::TokenStream {
    let syn::DeriveInput {
        data, attrs, ident, ..
    } = input;

    let impl_from_regex = match data {
        syn::Data::Enum(data_enum) => {
            impl_enum::Item::new(ident, &attrs, data_enum.variants.iter()).impl_from_regex()
        }
        syn::Data::Struct(syn::DataStruct { fields, .. }) => {
            impl_struct::Item::new(ident, &attrs, &fields).impl_from_regex()
        }
        _ => panic!("Unsupported item type"),
    };

    let impl_from_str = if impl_from_str {
        quote! {
            impl std::str::FromStr for #ident {
                type Err = ();
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Self::from_regex(s).ok_or(())
                }
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #impl_from_regex
        #impl_from_str
    }
}
