use std::collections::HashMap;

use lazy_static::lazy_static;
use quote::quote;
use regex::Regex;
use syn::spanned::Spanned;

pub type Groups<'a> = HashMap<&'a str, bool>;

// TODO: Check edges. Regex::capture_names might help, here, but it doesn't tell us if they're optional
const CAPTURE_GROUP_PATTERN: &str = r"[\(]*\?P<(?P<group>[A-z0-9_]+)>[^\)]*[\)]*(?P<optional>\?)?";
lazy_static! {
    static ref CAPTURE_GROUP_REGEX: Regex = Regex::new(CAPTURE_GROUP_PATTERN).unwrap();
}

pub fn from_regex_pattern(pat: &str) -> Groups {
    let groups = CAPTURE_GROUP_REGEX
        .captures_iter(pat)
        .map(|cap| {
            let name = cap.name("group").unwrap().as_str();
            let optional = cap.name("optional").is_some();
            (name, optional)
        })
        .collect::<HashMap<_, _>>();

    groups
}

// Use prefix to add a prefix to the capture group name (necessary for combined regex matching)
pub fn impl_fields_from_capture(
    captured_groups: &Groups,
    fields: &syn::Fields,
    prefix: Option<&str>,
) -> (Vec<syn::Ident>, Vec<proc_macro2::TokenStream>) {
    match fields {
        syn::Fields::Named(syn::FieldsNamed { named: fields, .. }) => {
            fields.iter().map(|field| {
                    let name = field.ident.clone().unwrap();
                    let name_val = if let Some(prefix) = prefix {
                        format!("{}_{}", prefix, name)
                    } else {
                        name.to_string()
                    };
                    let name_lit = syn::LitStr::new(&name_val, name.span());
                    let statement = match captured_groups.get(name_val.as_str()) {
                        Some(false) => quote! { let #name = captures.name(#name_lit).unwrap().as_str().into(); },
                        Some(true) => quote! { let #name = captures.name(#name_lit).map(|mat| mat.as_str().to_string()).into(); },
                        None => quote! { let #name = None; },
                    };
                    (name, statement)
                }).unzip::<_, _, Vec<_>, Vec<_>>()
        }

        syn::Fields::Unnamed(syn::FieldsUnnamed {
            unnamed: fields, ..
        }) => {
            fields.iter().enumerate().map(|(i, field)| {
                    let name_val = if let Some(prefix) = prefix {
                        format!("{}_{}", prefix, i)
                    } else {
                        format!("_{}", i)
                    };
                    let name_lit = syn::LitStr::new(&name_val, field.span());
                    let name = syn::Ident::new(&name_val, field.span());
                    let statement = match captured_groups.get(name_val.as_str()) {
                        Some(false) => quote! { let #name = captures.name(#name_lit).unwrap().as_str().into(); },
                        Some(true) => quote! { let #name = captures.name(#name_lit).map(|mat| mat.as_str().to_string()).into(); },
                        None => quote! { let #name = None; },
                    };
                    (name, statement)
                }).unzip::<_, _, Vec<_>, Vec<_>>()

        }

        syn::Fields::Unit => (Vec::new(), Vec::new()),
    }
}
