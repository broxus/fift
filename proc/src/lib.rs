use std::collections::HashMap;

use darling::{Error, FromMeta};
use proc_macro::TokenStream;
use quote::quote;
use syn::ItemImpl;

#[derive(Debug, FromMeta)]
struct FiftCmdArgs {
    #[darling(default)]
    tail: bool,
    #[darling(default)]
    active: bool,
    #[darling(default)]
    stack: bool,

    #[darling(default)]
    without_space: bool,

    name: String,

    #[darling(default)]
    args: Option<HashMap<String, syn::Expr>>,
}

#[proc_macro_attribute]
pub fn fift_module(_: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as ItemImpl);

    let dict_arg = quote::format_ident!("__dict");

    let mut definitions = Vec::new();
    let mut errors = Vec::new();

    let mut init_function_names = Vec::new();
    let mut init_functions = Vec::new();
    let mut other_functions = Vec::new();

    for impl_item in input.items.drain(..) {
        let syn::ImplItem::Fn(mut fun) = impl_item else {
            other_functions.push(impl_item);
            continue;
        };

        let mut has_init = false;

        let mut cmd_attrs = Vec::with_capacity(fun.attrs.len());
        let mut remaining_attr = Vec::new();
        for attr in fun.attrs.drain(..) {
            if let Some(path) = attr.meta.path().get_ident() {
                if path == "cmd" {
                    cmd_attrs.push(attr);
                    continue;
                } else if path == "init" {
                    has_init = true;
                    continue;
                }
            }

            remaining_attr.push(attr);
        }
        fun.attrs = remaining_attr;

        if has_init {
            fun.sig.ident = quote::format_ident!("__{}", fun.sig.ident);
            init_function_names.push(fun.sig.ident.clone());
            init_functions.push(fun);
        } else {
            for attr in cmd_attrs {
                match process_cmd_definition(&fun, &dict_arg, attr) {
                    Ok(definition) => definitions.push(definition),
                    Err(e) => errors.push(e),
                }
            }

            other_functions.push(syn::ImplItem::Fn(fun));
        }
    }

    if !errors.is_empty() {
        return TokenStream::from(Error::multiple(errors).write_errors());
    }

    let ty = input.self_ty;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics #ty #ty_generics #where_clause {
            #(#init_functions)*
        }

        #[automatically_derived]
        impl #impl_generics ::fift::core::Module for #ty #ty_generics #where_clause {
            fn init(
                &self,
                #dict_arg: &mut ::fift::core::Dictionary,
            ) -> ::core::result::Result<(), ::fift::error::Error> {
                #(self.#init_function_names(#dict_arg)?;)*
                #(#definitions?;)*
                Ok(())
            }
        }

        #(#other_functions)*
    }
    .into()
}

fn process_cmd_definition(
    function: &syn::ImplItemFn,
    dict_arg: &syn::Ident,
    attr: syn::Attribute,
) -> Result<syn::Expr, Error> {
    let cmd = FiftCmdArgs::from_meta(&attr.meta)?;

    let reg_fn = match (cmd.tail, cmd.active, cmd.stack) {
        (false, false, false) => quote! { define_context_word },
        (true, false, false) => quote! { define_context_tail_word },
        (false, true, false) => quote! { define_active_word },
        (false, false, true) => quote! { define_stack_word },
        _ => {
            return Err(Error::custom(
                "`tail`, `active` and `stack` cannot be used together",
            ));
        }
    };

    let cmd_name = if cmd.without_space {
        cmd.name.trim().to_owned()
    } else {
        format!("{} ", cmd.name.trim())
    };

    let function_name = function.sig.ident.clone();
    let expr = match cmd.args {
        None => {
            quote! { #function_name }
        }
        Some(provided_args) => {
            let ctx_arg = quote::format_ident!("__c");
            let required_args = find_command_args(function)?;

            let mut errors = Vec::new();
            let mut closure_args = vec![quote! { #ctx_arg }];
            for arg in required_args {
                match provided_args.get(&arg) {
                    Some(value) => closure_args.push(quote! { #value }),
                    None => errors.push(Error::custom(format!(
                        "No value provided for the argument `{arg}`"
                    ))),
                }
            }

            quote! { |#ctx_arg| #function_name(#(#closure_args),*)  }
        }
    };

    Ok(syn::parse_quote! { #dict_arg.#reg_fn(#cmd_name, #expr) })
}

fn find_command_args(function: &syn::ImplItemFn) -> Result<Vec<String>, Error> {
    let mut inputs = function.sig.inputs.iter();

    if let Some(first) = inputs.next() {
        if !matches!(first, syn::FnArg::Typed(_)) {
            return Err(Error::custom("Command context argument not found").with_span(&function));
        }
    }

    let mut args = Vec::new();
    for input in inputs {
        let syn::FnArg::Typed(input) = input else { continue };
        let syn::Pat::Ident(pat) = &*input.pat else {
            return Err(Error::custom("Unsupported argument binding").with_span(&input.pat));
        };
        args.push(pat.ident.to_string());
    }

    Ok(args)
}
