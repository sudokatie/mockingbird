//! Procedural macros for mockingbird.
//!
//! Provides the `#[mockingbird::test]` attribute macro for easy test setup.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitStr, Token};
use syn::parse::{Parse, ParseStream};

/// Arguments for the mockingbird test attribute.
struct TestArgs {
    cassette: String,
    mode: String,
}

impl Parse for TestArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut cassette = String::new();
        let mut mode = String::from("auto");

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match ident.to_string().as_str() {
                "cassette" => cassette = value.value(),
                "mode" => mode = value.value(),
                _ => return Err(syn::Error::new(ident.span(), "unknown argument")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        if cassette.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "cassette argument is required",
            ));
        }

        Ok(TestArgs { cassette, mode })
    }
}

/// Test attribute macro for mockingbird.
///
/// Automatically sets up a mockingbird client for your test.
///
/// # Example
///
/// ```ignore
/// #[mockingbird::test(cassette = "tests/cassettes/api.json", mode = "auto")]
/// async fn test_api_call(client: mockingbird::Client) {
///     let response = client.get("https://api.example.com/users").send().await.unwrap();
///     assert_eq!(response.status(), 200);
/// }
/// ```
///
/// # Arguments
///
/// - `cassette` (required): Path to the cassette file
/// - `mode` (optional): Operating mode - "record", "playback", or "auto" (default: "auto")
#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as TestArgs);
    let input_fn = parse_macro_input!(input as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let fn_block = &input_fn.block;
    let fn_attrs = &input_fn.attrs;
    let fn_vis = &input_fn.vis;

    let cassette_path = &args.cassette;
    let mode_str = &args.mode;

    // Generate the mode constructor call
    let mode_constructor = match mode_str.as_str() {
        "record" => quote! { mockingbird::Client::record(#cassette_path) },
        "playback" | "replay" => quote! { mockingbird::Client::playback(#cassette_path) },
        _ => quote! { mockingbird::Client::auto(#cassette_path) },
    };

    let expanded = quote! {
        #(#fn_attrs)*
        #[::tokio::test]
        #fn_vis async fn #fn_name() {
            let client = #mode_constructor
                .build()
                .expect("Failed to create mockingbird client");
            
            // Allow the test body to use `client`
            let __mockingbird_test_impl = async move {
                let client = client;
                #fn_block
            };
            
            __mockingbird_test_impl.await
        }
    };

    TokenStream::from(expanded)
}
