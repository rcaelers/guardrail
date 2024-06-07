extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{parse_macro_input, DeriveInput, Ident, Type};

fn expand_derive_dtos(input: DeriveInput) -> syn::Result<TokenStream> {
    let fields = match input.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(syn::FieldsNamed { named, .. }),
            ..
        }) => named,
        _ => {
            return Ok(quote_spanned! {
                input.ident.span() => compile_error!("you can only derive DeriveDtoModel on structs");
            })
        }
    };

    let create_ident = format_ident!("Create{}", input.ident);
    let update_ident = format_ident!("Update{}", input.ident);

    let mut field_idents: Vec<Ident> = Vec::new();
    let mut field_types: Vec<Type> = Vec::new();
    let mut id_field_idents: Vec<Ident> = Vec::new();
    let mut id_field_types: Vec<Type> = Vec::new();
    let mut id_init_create = quote! {};
    let mut id_init_update = quote! {};

    for field in fields {
        if let Some(ident) = &field.ident {
            let field_type = &field.ty;
            let field_type = quote! { #field_type }.to_string().replace(' ', "");

            if ident == "id" && field_type == "Uuid" {
                id_field_idents.push(ident.clone());
                id_field_types.push(field.ty.clone());
                id_init_create = quote! { id: sea_orm::Set(uuid::Uuid::new_v4()), };
                id_init_update = quote! { id: sea_orm::Set(self.id), };
            }

            if !((ident == "id" && field_type == "Uuid")
                || ident == "created_at"
                || ident == "updated_at")
            {
                field_idents.push(ident.clone());
                field_types.push(field.ty);
            }
        }
    }

    let ts = quote!(
      #[automatically_derived]
      #[derive(Clone, Debug, Deserialize, Serialize)]
      pub struct #create_ident {
          #(
              pub #field_idents: #field_types
          ),*
      }

      #[automatically_derived]
      impl #create_ident {
          pub fn new(#(#field_idents: #field_types),*) -> Self {
              Self {
                  #(
                    #field_idents
                  ),*
              }
          }
      }

      #[automatically_derived]
      impl sea_orm::IntoActiveModel<ActiveModel> for #create_ident {
        fn into_active_model(self) -> ActiveModel {
            let now = chrono::Utc::now().naive_utc();
            ActiveModel {
                #id_init_create
                #(
                  #field_idents: sea_orm::Set(self.#field_idents)
                ),*,
                created_at: sea_orm::Set(now),
                updated_at: sea_orm::Set(now),
            }
        }
      }

      #[automatically_derived]
      #[derive(Clone, Debug, Deserialize, Serialize)]
      pub struct #update_ident {
          #(
              pub #id_field_idents: #id_field_types,
          )*
          #(
              pub #field_idents: #field_types
          ),*
      }

      #[automatically_derived]
      impl #update_ident {
          pub fn new(#(#id_field_idents: #id_field_types,)* #(#field_idents: #field_types,)*) -> Self {
              Self {
                  #(
                    #id_field_idents,
                  )*
                  #(
                    #field_idents
                  ),*
              }
          }
      }
      #[automatically_derived]
      impl sea_orm::IntoActiveModel<ActiveModel> for #update_ident {
        fn into_active_model(self) -> ActiveModel {
            let now = chrono::Utc::now().naive_utc();
            ActiveModel {
                #id_init_update
                #(#field_idents: sea_orm::Set(self.#field_idents),)*
                created_at: sea_orm::NotSet,
                updated_at: sea_orm::Set(now),
            }
        }
      }
    );
    Ok(ts)
}

#[proc_macro_derive(DeriveDtoModel)]
pub fn derive_dto(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_derive_dtos(input) {
        Ok(token_stream) => token_stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
