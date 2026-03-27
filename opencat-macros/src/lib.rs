use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, PatType, ReturnType, Type, TypePath};

#[derive(Debug, Default, FromMeta)]
struct ComponentArgs {}

#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    if let Err(e) = ComponentArgs::from_list(&args) {
        return TokenStream::from(e.write_errors());
    }

    let input = parse_macro_input!(item as ItemFn);

    if let Err(e) = validate_component_signature(&input) {
        return TokenStream::from(e);
    }

    TokenStream::from(quote! { #input })
}

fn validate_component_signature(input: &ItemFn) -> Result<(), proc_macro2::TokenStream> {
    if input.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "#[component] function must have exactly one parameter: &FrameCtx",
        )
        .to_compile_error());
    }

    let Some(first_arg) = input.sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "#[component] function must have exactly one parameter: &FrameCtx",
        )
        .to_compile_error());
    };

    let FnArg::Typed(PatType { ty, .. }) = first_arg else {
        return Err(syn::Error::new_spanned(
            first_arg,
            "#[component] does not support methods with self receiver",
        )
        .to_compile_error());
    };

    let Type::Reference(reference) = ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            ty,
            "first parameter of #[component] must be &FrameCtx",
        )
        .to_compile_error());
    };

    let Type::Path(TypePath { path, .. }) = reference.elem.as_ref() else {
        return Err(syn::Error::new_spanned(
            &reference.elem,
            "first parameter of #[component] must be &FrameCtx",
        )
        .to_compile_error());
    };

    let is_frame_ctx = path
        .segments
        .last()
        .map(|seg| seg.ident == "FrameCtx")
        .unwrap_or(false);

    if !is_frame_ctx {
        return Err(syn::Error::new_spanned(
            path,
            "first parameter of #[component] must be &FrameCtx",
        )
        .to_compile_error());
    }

    let ReturnType::Type(_, ret_ty) = &input.sig.output else {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "#[component] function must return Node",
        )
        .to_compile_error());
    };

    let Type::Path(TypePath { path, .. }) = ret_ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            ret_ty,
            "#[component] function must return Node",
        )
        .to_compile_error());
    };

    let is_node = path
        .segments
        .last()
        .map(|seg| seg.ident == "Node")
        .unwrap_or(false);

    if !is_node {
        return Err(syn::Error::new_spanned(
            path,
            "#[component] function must return Node",
        )
        .to_compile_error());
    }

    Ok(())
}
