use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType};

#[proc_macro_attribute]
pub fn redox_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let attrs = &input.attrs;
    let vis = &input.vis;
    let mut sig = input.sig.clone();
    let block = &input.block;
    let fn_name = &sig.ident;

    // Ensure the exported function has C ABI
    sig.abi = Some(syn::parse_quote!(extern "C"));

    let body = match &sig.output {
        ReturnType::Default => {
            quote! {
                if let Err(err) = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #block)) {
                    ::redox_mc_api::macros::handle_panic(stringify!(#fn_name), err);
                }
            }
        }
        ReturnType::Type(_, ty) => {
            quote! {
                match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #block)) {
                    Ok(res) => res,
                    Err(err) => {
                        ::redox_mc_api::macros::handle_panic(stringify!(#fn_name), err);
                        <#ty as ::redox_mc_api::macros::PanicFallback>::panic_fallback()
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #(#attrs)*
        #[no_mangle]
        #vis #sig {
            #body
        }
    };

    TokenStream::from(expanded)
}
