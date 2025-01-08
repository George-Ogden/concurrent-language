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
                    let field_type_name = quote!(#field_type).to_string();

                    if field_type_name == variant_name.to_string() {
                        impls.push(quote! {
                            impl From<#field_type> for #name {
                                fn from(value: #field_type) -> Self {
                                    #name::#variant_name(value)
                                }
                            }
                        });
                    }
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
