use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(FromVariants)]
pub fn derive_from_variants(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut impls = Vec::new();

    if let Data::Enum(data_enum) = input.data {
        for variant in data_enum.variants {
            let variant_name = variant.ident;

            if let Fields::Unnamed(fields) = variant.fields {
                if fields.unnamed.len() == 1 {
                    let field_type = &fields.unnamed[0].ty;

                    impls.push(quote! {
                        impl From<#field_type> for #name {
                            fn from(value: #field_type) -> Self {
                                #name::#variant_name(value)
                            }
                        }
                    });
                } else {
                    panic!("Only single unnamed fields in variants are supported");
                }
            }
        }
    } else {
        panic!("This macro only works on enums");
    }

    let output = quote! {
        #(#impls)*
    };

    output.into()
}
