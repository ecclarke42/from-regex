use heck::ShoutySnekCase;
use quote::quote;
use syn::spanned::Spanned;

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
        for attr in attrs {
            if let syn::Meta::List(list) = attr.parse_meta().expect("failed to parse attr meta") {
                if list.path.is_ident(crate::ATTRIBUTE) {
                    for nested in list.nested {
                        if let syn::NestedMeta::Meta(syn::Meta::NameValue(syn::MetaNameValue {
                            path,
                            lit: syn::Lit::Str(lit),
                            ..
                        })) = nested
                        {
                            if path.is_ident(ITEM_ATTRIBUTE_PATTERN) {
                                pattern = Some(lit);
                            }
                        }
                    }
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

    pub fn impl_from_regex(&self) -> proc_macro2::TokenStream {
        let ident = self.ident;
        let pattern = self.attrs.pattern.value();
        let regex_const = syn::Ident::new(
            &format!("{}_REGEX", self.ident.to_string().TO_SHOUTY_SNEK_CASE()),
            self.ident.span(),
        );

        let (from_capture_fn_ident, from_capture_impl) =
            self.impl_from_capture(crate::captures::from_regex_pattern(&pattern));
        let (from_regex_as_fn_ident, from_regex_as_impl) =
            self.impl_from_regex_as_exact(&regex_const);

        let search_impl = match self.fields {
            syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                quote! {
                    pub fn search(s: &str) -> Vec<(Self, Range<usize>)> {
                        #regex_const
                            .captures_iter(&s)
                            .filter_map(|cap| {
                                let range = cap.get(0).unwrap().range(); // Unwrap should be fine, since otherwise it wouldn't match
                                let value = Self::#from_capture_fn_ident(cap);
                                value.map(|v| (range, v))
                            }).collect()
                    }
                }
            }
            syn::Fields::Unit => {
                quote! {
                    pub fn search(s: &str) -> Vec<Range<usize>> {
                        #regex_const
                            .find_iter(&s)
                            .map(|mat| mat.range())
                            .collect();
                    }
                }
            }
        };

        quote! {
            from_regex::lazy_static! {
                static ref #regex_const: from_regex::Regex = from_regex::Regex::new(#pattern).expect("Failed to compile regex");
            }
            impl #ident {
                #from_capture_impl
                #from_regex_as_impl
                // #regex_capture_consuming_impl

                pub fn from_regex(s: &str) -> Option<Self> {
                    Self::#from_regex_as_fn_ident(s)
                }

                #search_impl
            }
        }
    }

    fn from_capture_fn_ident(&self) -> syn::Ident {
        syn::Ident::new("from_capture", self.ident.span())
    }

    fn from_regex_fn_ident(&self) -> syn::Ident {
        syn::Ident::new("from_regex", self.ident.span())
    }

    fn impl_from_capture(
        &self,
        captured_fields: crate::captures::Groups,
    ) -> (syn::Ident, proc_macro2::TokenStream) {
        let fn_ident = self.from_capture_fn_ident();

        let implementation = match self.fields {
            syn::Fields::Named(syn::FieldsNamed { named: fields, .. }) => {
                let (field_names, field_statements) = fields.into_iter().map(|field| {
            let name = field.ident.clone().unwrap();
            let name_val = name.to_string();
            let name_lit = syn::LitStr::new(&name_val, name.span());

            let statement = match captured_fields.get(name_val.as_str()) {
                Some(false) => quote! { let #name = captures.name(#name_lit).unwrap().as_str().into(); },
                Some(true) => quote! { let #name = captures.name(#name_lit).map(|mat| mat.as_str().to_string()).into(); },
                None => quote! { let #name = None; },
            };
            (name, statement)

        }).unzip::<_, _, Vec<_>, Vec<_>>();

                quote! {
                    fn #fn_ident(captures: from_regex::Captures) -> Option<Self> {
                        #(#field_statements)*
                        Some(Self{ #(#field_names),* })
                    }
                }
            }

            syn::Fields::Unnamed(syn::FieldsUnnamed {
                unnamed: fields, ..
            }) => {
                let (assigned_names, field_statements) = fields.into_iter().enumerate().map(|(i, field)| {
            let name_val = format!("_{}", i);
            let name_lit = syn::LitStr::new(&name_val, field.span());
            let name = syn::Ident::new(&name_val, field.span());

            let statement = match captured_fields.get(name_val.as_str()) {
                Some(false) => quote! { let #name = captures.name(#name_lit).unwrap().as_str().into(); },
                Some(true) => quote! { let #name = captures.name(#name_lit).map(|mat| mat.as_str().to_string()).into(); },
                None => quote! { let #name = None; },
            };
            (name, statement)
            
        }).unzip::<_, _, Vec<_>, Vec<_>>();

                quote! {
                    fn #fn_ident(captures: from_regex::Captures) -> Option<Self> {
                        #(#field_statements)*
                        Some(Self( #(#assigned_names),* ))
                    }
                }
            }

            syn::Fields::Unit => {
                quote! {
                    fn #fn_ident(captures: from_regex::Captures) -> Option<Self> {
                        Some(Self)
                    }
                }
            }
        };

        (fn_ident, implementation)
    }

    fn impl_from_regex_as_exact(
        &self,
        regex_const: &syn::Ident,
    ) -> (syn::Ident, proc_macro2::TokenStream) {
        let fn_ident = self.from_regex_fn_ident();
        let from_capture_fn_ident = self.from_capture_fn_ident();

        let implementation = match self.fields {
            syn::Fields::Named(_) | syn::Fields::Unnamed(_) => quote! {
                fn #fn_ident(s: &str) -> Option<Self> {
                    match #regex_const.captures(s) {
                        Some(cap) if cap[0].len() == s.len() => Self::#from_capture_fn_ident(cap),
                        Some(_) => None,
                        None => None,
                    }
                }
            },
            syn::Fields::Unit => quote! {
                fn #fn_ident(s: &str) -> Option<Self> {
                    if #regex_const.is_match(s) {
                        Some(Self)
                    } else {
                        None
                    }
                }
            },
        };

        (fn_ident, implementation)
    }
}
