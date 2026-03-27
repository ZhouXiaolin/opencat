use darling::{FromMeta, ast::NestedMeta};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, PatType, ReturnType, Type, TypePath, parse_macro_input};

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

    if input.sig.inputs.len() == 1 {
        return TokenStream::from(quote! { #input });
    }

    TokenStream::from(expand_param_component(input))
}

fn validate_component_signature(input: &ItemFn) -> Result<(), proc_macro2::TokenStream> {
    if input.sig.inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "#[component] function must start with &FrameCtx",
        )
        .to_compile_error());
    }

    let Some(first_arg) = input.sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "#[component] function must start with &FrameCtx",
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
        return Err(
            syn::Error::new_spanned(&input.sig, "#[component] function must return Node")
                .to_compile_error(),
        );
    };

    let Type::Path(TypePath { path, .. }) = ret_ty.as_ref() else {
        return Err(
            syn::Error::new_spanned(ret_ty, "#[component] function must return Node")
                .to_compile_error(),
        );
    };

    let is_node = path
        .segments
        .last()
        .map(|seg| seg.ident == "Node")
        .unwrap_or(false);

    if !is_node {
        return Err(
            syn::Error::new_spanned(path, "#[component] function must return Node")
                .to_compile_error(),
        );
    }

    Ok(())
}

fn expand_param_component(input: ItemFn) -> proc_macro2::TokenStream {
    let vis = &input.vis;
    let factory_name = &input.sig.ident;
    let impl_name = format_ident!("__opencat_component_impl_{}", factory_name);
    let mut impl_fn = input.clone();
    impl_fn.sig.ident = impl_name.clone();

    let props = input
        .sig
        .inputs
        .iter()
        .skip(1)
        .map(extract_prop)
        .collect::<Result<Vec<_>, _>>();

    let props = match props {
        Ok(props) => props,
        Err(err) => return err.to_compile_error(),
    };

    let prop_bindings = props.iter().map(|(ident, ty)| quote! { #ident: #ty });
    let prop_idents = props.iter().map(|(ident, _)| ident);
    let cloned_props = prop_idents
        .clone()
        .map(|ident| quote! { ::core::clone::Clone::clone(&#ident) });

    quote! {
        #impl_fn

        #vis fn #factory_name(#(#prop_bindings),*) -> ::opencat::Node {
            ::opencat::component_node(move |__opencat_ctx| {
                #impl_name(
                    __opencat_ctx,
                    #(#cloned_props),*
                )
            })
        }
    }
}

fn extract_prop(arg: &FnArg) -> Result<(syn::Ident, Type), syn::Error> {
    let FnArg::Typed(PatType { pat, ty, .. }) = arg else {
        return Err(syn::Error::new_spanned(
            arg,
            "#[component] does not support methods with self receiver",
        ));
    };

    let Pat::Ident(ident) = pat.as_ref() else {
        return Err(syn::Error::new_spanned(
            pat,
            "#[component] props must be simple named parameters",
        ));
    };

    Ok((ident.ident.clone(), (*ty.clone())))
}
