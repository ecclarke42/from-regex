use heck::{ShoutySnekCase, SnekCase};
use quote::quote;
use syn::spanned::Spanned;

pub struct Item<'a> {
    ident: &'a syn::Ident,
    attrs: ItemAttributes,
    variants: Vec<Variant<'a>>,
}

pub struct ItemAttributes;
impl From<&[syn::Attribute]> for ItemAttributes {
    fn from(_: &[syn::Attribute]) -> Self {
        Self
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

    pub fn impl_from_regex(&self) -> proc_macro2::TokenStream {
        let ident = self.ident;
        let enum_name_shouty = self.ident.to_string().TO_SHOUTY_SNEK_CASE();

        // Because these are annoying to unzip a bunch of times
        let mut regex_defs = Vec::new();
        let mut from_capture_fn_idents = Vec::new();
        let mut from_capture_impls = Vec::new();
        let mut from_regex_as_fn_idents = Vec::new();
        let mut from_regex_as_impls = Vec::new();
        let mut regex_capture_consuming_fn_idents = Vec::new();
        let mut regex_capture_consuming_impls = Vec::new();

        let mut default_variant = None;
        for variant in self.variants.iter() {
            if variant.attrs.default {
                // TODO: Parse type here?
                if default_variant.is_some() {
                    panic!("More than one default varaiant");
                }
                default_variant = Some(variant);
            }

            if let Some(pattern) = &variant.attrs.pattern {
                let pattern = pattern.value();
                let capture_groups = crate::captures::from_regex_pattern(&pattern);
                let regex_const = syn::Ident::new(
                    &format!(
                        "{}_{}_REGEX",
                        enum_name_shouty,
                        variant.ident.to_string().TO_SHOUTY_SNEK_CASE()
                    ),
                    self.ident.span(),
                );

                regex_defs.push(quote! {
                    static ref #regex_const: from_regex::Regex = from_regex::Regex::new(#pattern).expect("Failed to compile regex");
                });

                let (ident, implementation) = 
                    variant.impl_from_capture(capture_groups);
                from_capture_fn_idents.push(ident);
                from_capture_impls.push(implementation);

                let (ident, implementation) = 
                    variant.impl_from_regex_as_exact(&regex_const);
                from_regex_as_fn_idents.push(ident);
                from_regex_as_impls.push(implementation);

                let (ident, implementation) = 
                    variant.impl_capture_consuming(&regex_const);
                regex_capture_consuming_fn_idents.push(ident);
                regex_capture_consuming_impls.push(implementation);
            }
        }

        // TODO: improve search to include ranges as well
        quote! {
            from_regex::lazy_static! {
                #(#regex_defs)*
            }
            impl #ident {
                #(#from_capture_impls)*
                #(#from_regex_as_impls)*
                #(#regex_capture_consuming_impls)*

                pub fn from_regex(s: &str) -> Option<Self> {
                    #(
                        if let Some(variant) = Self::#from_regex_as_fn_idents(s) {
                            return Some(variant);
                        }
                    )*
                    None
                }

                /// Note: this will allocate a new string from `s`, which will be consumed as items are consumed
                pub fn search(s: &str) -> Vec<Self> {
                    let mut s = s.to_string();
                    let mut out = Vec::new();
                    #(
                        out.append(&mut Self::#regex_capture_consuming_fn_idents(&mut s));
                    )*
                    out
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
    pattern: Option<syn::LitStr>,
    default: bool,
}
const VARIANT_ATTRIBUTE_PATTERN: &str = "pattern";
const VARIANT_ATTRIBUTE_DEFAULT: &str = "default";

impl<'a> From<&'a [syn::Attribute]> for VariantAttributes {
    fn from(attrs: &'a [syn::Attribute]) -> Self {
        let mut pattern = None;
        let mut default = false;
        for attr in attrs {
            if let syn::Meta::List(list) = attr.parse_meta().expect("failed to parse attr meta") {
                if list.path.is_ident(crate::ATTRIBUTE) {
                    for nested in list.nested {
                        if let syn::NestedMeta::Meta(meta) = nested {
                            match meta {
                                syn::Meta::NameValue(syn::MetaNameValue {
                                    path,
                                    lit: syn::Lit::Str(lit),
                                    ..
                                }) => {
                                    if path.is_ident(VARIANT_ATTRIBUTE_PATTERN) {
                                        pattern = Some(lit);
                                    }
                                }
                                syn::Meta::Path(path) => {
                                    if path.is_ident(VARIANT_ATTRIBUTE_DEFAULT) {
                                        default = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Self {
            pattern,
            default,
        }
    }
}

impl<'a> Variant<'a> {
    pub fn new(variant: &'a syn::Variant) -> Self {
        Self {
            ident: &variant.ident,
            attrs: VariantAttributes::from(variant.attrs.as_ref()),
            fields: &variant.fields,
        }
    }

    fn from_capture_fn_ident(&self) -> syn::Ident {
        syn::Ident::new(&format!("from_{}_capture", self.ident.to_string().to_snek_case()), self.ident.span())
    }

    fn from_regex_fn_ident(&self) -> syn::Ident {
        syn::Ident::new(&format!("from_regex_as_{}", self.ident.to_string().to_snek_case()), self.ident.span())
    }

    fn capture_consuming_fn_ident(&self) -> syn::Ident {
        syn::Ident::new(&format!("regex_capture_{}_consuming", self.ident.to_string().to_snek_case()), self.ident.span())
    }

    pub fn impl_from_capture(&self, captured_fields: crate::captures::Groups) -> (syn::Ident, proc_macro2::TokenStream) {
        let variant = self.ident;
        let fn_ident = self.from_capture_fn_ident();

        let implementation = match self.fields {

            syn::Fields::Named(syn::FieldsNamed { named: fields, ..}) => {

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
                            Some(Self::#variant { #(#field_names),* })
                        }
                    }
                
            }


            syn::Fields::Unnamed(syn::FieldsUnnamed {unnamed: fields, .. }) => {
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
                            Some(Self::#variant ( #(#assigned_names),* ))
                        }
                    }
            }

            syn::Fields::Unit => quote! {
                fn #fn_ident(captures: from_regex::Captures) -> Option<Self> {
                    Some(Self::#variant)
                }
            },
        };

        (fn_ident,
            implementation
        )

    }

    
fn impl_from_regex_as_exact(
    &self,
    regex_const: &syn::Ident,
) -> (syn::Ident, proc_macro2::TokenStream) {
    let variant = self.ident;
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
                    Some(Self::#variant)
                } else {
                    None
                }
            }
        }
    };

    (fn_ident, implementation)
}

// Only for enum variant (struct searches directly)
fn impl_capture_consuming(&self,
    regex_const: &syn::Ident) ->(syn::Ident, proc_macro2::TokenStream) {
        let variant = self.ident;
        let define_ranges_and_values = match self.fields {
            syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                let from_capture_fn_ident = self.from_capture_fn_ident();
                quote! {
                    let (ranges, values) = #regex_const
                        .captures_iter(&s)
                        .filter_map(|cap| {
                            let range = cap.get(0).unwrap().range(); // Unwrap should be fine, since otherwise it wouldn't match
                            let value = Self::#from_capture_fn_ident(cap);
                            value.map(|v| (range, v))
                        })
                        .unzip::<_, _, Vec<_>, Vec<_>>();
                }
            },
            syn::Fields::Unit => quote! {
                    let (ranges, values) = #regex_const
                        .find_iter(&s)
                        .map(|mat| (mat.range(), Self::#variant))
                        .unzip::<_, _, Vec<_>, Vec<_>>();
                }
        };
    
        let fn_ident = self.capture_consuming_fn_ident();
        let implementation = quote! {

            fn #fn_ident(s: &mut String) -> Vec<Self> {
                let mut offset = 0;
            
                #define_ranges_and_values

                // Remove these ranges from the string (after collecting so s isn't borrowed in the capture)
                for mut range in ranges {
                    range.start -= offset;
                    range.end -= offset;
                    offset += range.len();
                    s.replace_range(range, "");
                }

                values
            }
        };

        (fn_ident, implementation)
    }
}