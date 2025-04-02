use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(FromVariants)]
/// Automatically generate a `.into()` methods for all structs that share their field name.
pub fn derive_from_variants(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut implementations = Vec::new();

    if let Data::Enum(data_enum) = input.data {
        for variant in data_enum.variants {
            let variant_name = variant.ident;

            if let Fields::Unnamed(fields) = variant.fields {
                // Check for a single unnamed field.
                if fields.unnamed.len() == 1 {
                    let field_type = &fields.unnamed[0].ty;
                    let field_type_name = quote!(#field_type).to_string();

                    // Check the field matches the type.
                    if field_type_name == variant_name.to_string() {
                        implementations.push(quote! {
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
        #(#implementations)*
    };

    output.into()
}
