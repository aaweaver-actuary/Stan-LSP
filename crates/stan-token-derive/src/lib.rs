//! Derive support for enums whose variants have fixed textual spellings.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Generics, Ident, LitStr, Result, Variant, Visibility};

#[proc_macro_derive(Token, attributes(token))]
pub fn derive_token(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    expand_token(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Clone)]
struct TokenVariant {
    ident: Ident,
    literal: LitStr,
    value: String,
}

fn expand_token(input: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ident,
        vis,
        generics,
        data,
        ..
    } = input;

    reject_generics(&generics)?;

    let data = match data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                &ident,
                "Token can only be derived for enums",
            ));
        }
    };

    let mut seen = HashMap::<String, LitStr>::new();
    let mut variants = Vec::with_capacity(data.variants.len());
    for variant in data.variants {
        let token = parse_variant(&variant)?;
        if let Some(first) = seen.insert(token.value.clone(), token.literal.clone()) {
            let mut error = syn::Error::new_spanned(
                &token.literal,
                format!("duplicate token value {:?}", token.value),
            );
            error.combine(syn::Error::new_spanned(first, "first declared here"));
            return Err(error);
        }
        variants.push(token);
    }

    generate(&ident, &vis, &variants)
}

fn reject_generics(generics: &Generics) -> Result<()> {
    if generics.params.is_empty() && generics.where_clause.is_none() {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            generics,
            "Token enums cannot have generic parameters or where clauses",
        ))
    }
}

fn parse_variant(variant: &Variant) -> Result<TokenVariant> {
    if !matches!(variant.fields, Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &variant.ident,
            "Token can only be derived for unit variants",
        ));
    }

    let attributes = variant
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("token"))
        .collect::<Vec<_>>();

    let attribute = match attributes.as_slice() {
        [] => {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "missing #[token(\"...\")] attribute",
            ));
        }
        [attribute] => *attribute,
        _ => {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "expected exactly one #[token(\"...\")] attribute",
            ));
        }
    };

    let literal = attribute.parse_args::<LitStr>()?;
    let value = literal.value();
    if value.is_empty() {
        return Err(syn::Error::new_spanned(
            &literal,
            "token value cannot be empty",
        ));
    }

    Ok(TokenVariant {
        ident: variant.ident.clone(),
        literal,
        value,
    })
}

fn generate(enum_name: &Ident, vis: &Visibility, variants: &[TokenVariant]) -> Result<TokenStream> {
    let error_name = format_ident!("{}ParseError", enum_name);
    let variant_names = variants.iter().map(|variant| &variant.ident);
    let as_str_arms = variants.iter().map(|variant| {
        let name = &variant.ident;
        let literal = &variant.literal;
        quote!(Self::#name => #literal,)
    });
    let from_str_arms = variants.iter().map(|variant| {
        let name = &variant.ident;
        let literal = &variant.literal;
        quote!(#literal => ::core::result::Result::Ok(Self::#name),)
    });

    Ok(quote! {
        #[derive(Debug, Clone, PartialEq, Eq)]
        #vis struct #error_name {
            input: ::std::string::String,
        }

        impl #error_name {
            #vis fn input(&self) -> &str {
                &self.input
            }
        }

        impl ::core::fmt::Display for #error_name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(
                    formatter,
                    "unknown {} token {:?}",
                    stringify!(#enum_name),
                    self.input,
                )
            }
        }

        impl ::std::error::Error for #error_name {}

        impl #enum_name {
            #vis const ALL: &'static [Self] = &[#(Self::#variant_names),*];

            #vis const fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms)*
                }
            }

            #vis fn from_str(input: &str) -> ::core::result::Result<Self, #error_name> {
                match input {
                    #(#from_str_arms)*
                    _ => ::core::result::Result::Err(#error_name {
                        input: ::std::string::String::from(input),
                    }),
                }
            }
        }

        impl ::core::str::FromStr for #enum_name {
            type Err = #error_name;

            fn from_str(input: &str) -> ::core::result::Result<Self, Self::Err> {
                #enum_name::from_str(input)
            }
        }

        impl ::core::fmt::Display for #enum_name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::core::convert::AsRef<str> for #enum_name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl ::core::convert::TryFrom<&str> for #enum_name {
            type Error = #error_name;

            fn try_from(input: &str) -> ::core::result::Result<Self, Self::Error> {
                #enum_name::from_str(input)
            }
        }
    })
}
