use std::collections::HashSet;

use quote::quote;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Spacing, Span, TokenTree};
use syn::{Ident, ItemEnum};

#[proc_macro_derive(
    NetworkCsmStateTransition,
    attributes(network_csm_state_transition, network_csm_client)
)]
pub fn derive_network_state(input: TokenStream) -> TokenStream {
    // parse the messages enum
    let messages = syn::parse_macro_input!(input as ItemEnum);

    // parse the attributes for state transition and other parameters
    let attr = messages.attrs.into_iter().find(|attr| {
        attr.meta.path().get_ident()
            == Some(&Ident::new(
                "network_csm_state_transition",
                Span::call_site(),
            ))
    });

    let Some(attr) = attr else {
        panic!(
            "cannot do NetworkCsmStateTransitions, without a network_csm_state_transitions attribute"
        )
    };

    let (state_ident, transitions) = match attr.meta {
        syn::Meta::Path(_) | syn::Meta::NameValue(_) => {
            panic!("expected list in attribute")
        }
        syn::Meta::List(meta_list) => {
            let mut tokens = meta_list.tokens.into_iter();
            let state_ident_token = tokens.next().expect("expecting state ident");
            let punct_token = tokens.next().expect("token");
            let group_token = tokens.next().expect("list");

            let state_ident = get_ident(state_ident_token).unwrap();
            is_punct(punct_token, ',', Spacing::Alone).unwrap();
            let group = get_group(group_token, Delimiter::Bracket).unwrap();

            let transitions = parse_transitions(group);
            (state_ident, transitions)
        }
    };

    let context = Context {
        state_name: state_ident,
        msg_name: messages.ident,
        transitions,
    };

    // verify transition tables
    for trans in context.transitions.iter() {
        let msg = &trans.message;
        let Some(_) = messages.variants.iter().find(move |m| &m.ident == msg) else {
            panic!("cannot find message {} define at {:?}", msg, msg.span())
        };
    }

    let has_client_attr = |v: &syn::Variant| {
        v.attrs
            .iter()
            .any(|a| a.path().is_ident("network_csm_client"))
    };
    let (client_messages, _server_messages) = messages
        .variants
        .iter()
        .partition::<Vec<&syn::Variant>, _>(|v| has_client_attr(v));

    let client_match_fns = client_messages
        .iter()
        .filter_map(|client_msg| client_msg_generate(&context, client_msg, &messages.variants))
        .collect::<Vec<_>>();

    let server_states = client_messages
        .iter()
        .map(|client_msg| {
            context
                .transitions_for_message(&client_msg.ident)
                .map(|t| t.start.clone())
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<HashSet<_>>();

    let st_and_transition_msgs = server_states
        .iter()
        .map(|st| {
            let msgs = context
                .transitions_messages_starts_with_state(st)
                .collect::<Vec<_>>();
            (
                st,
                msgs.into_iter()
                    .map(|m1| {
                        messages
                            .variants
                            .iter()
                            .find(move |m2| m1 == &m2.ident)
                            .expect("message found")
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    let server_match_fns = st_and_transition_msgs
        .iter()
        .map(|(st, msgs)| server_msg_generate(&context, st, msgs))
        .collect::<Vec<_>>();

    let transition_fn = generate_transition_fn(&context, &messages.variants);
    quote! {
        #transition_fn
        #(#client_match_fns)*
        #(#server_match_fns)*
    }
    .into()
}

/// Generate a function that give a message and a state, will gives you the next state
///
/// If this combinaison is invalid, then None is return
fn generate_transition_fn(
    context: &Context,
    messages: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    let impl_name = &context.msg_name;
    let state_ident = &context.state_name;

    let body = messages
        .iter()
        .map(|v| {
            let found_trans = context
                .transitions_for_message(&v.ident)
                .collect::<Vec<_>>();
            let id = &v.ident;
            let msg_params = if v.fields.is_empty() {
                quote! {}
            } else {
                quote! { (..) }
            };
            if found_trans.is_empty() {
                panic!(
                    "message {:?} with no state transition, most likely a bug in the definition",
                    id
                )
            } else {
                let trans = found_trans
                    .iter()
                    .map(|transition| {
                        let state_start = &transition.start;
                        let state_end = &transition.end;
                        quote! {
                            #state_ident :: #state_start => Some( #state_ident :: #state_end ),
                        }
                    })
                    .collect::<Vec<_>>();
                quote! {
                    #impl_name :: #id #msg_params => {
                        match current_state {
                            #(#trans)*
                            _ => None,
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        impl #impl_name {
            fn can_transition(&self, current_state: #state_ident) -> Option<#state_ident> {
                match self {
                    #(#body)*
                }
            }
        }
    }
}

/// Generate client API related side
///
/// * client_<message>_ret which parse the valid expected type and extract it in the call's specific return values
///
/// For Message that may result in multiple replies, also generate:
///
/// * A enum type <Message>Ret that contains only the specific valid variants associated with the return value
/// * A From <MessageRet> to the Message type
///
fn client_msg_generate(
    context: &Context,
    v: &syn::Variant,
    messages: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
) -> Option<proc_macro2::TokenStream> {
    let impl_name = &context.msg_name;

    let found_trans = context
        .transitions_for_message(&v.ident)
        .collect::<Vec<_>>();

    if found_trans.len() == 0 || found_trans.len() > 1 {
        panic!(
            "transition should have only 1 outcome: found {:?}",
            found_trans
        )
    }
    let end = &found_trans[0].end;

    let ret_possible = context
        .transitions_messages_starts_with_state(&end)
        .collect::<Vec<_>>();

    if ret_possible.is_empty() {
        //panic!("ret possible is empty {}", v.ident)
        return None;
    }

    let need_enum = ret_possible.len() > 1;

    //panic!("ret possible {:?}", ret_possible);

    let fn_name = quote::format_ident!("client_{}_ret", camel_to_snake(&v.ident.to_string()));
    let ret_variants = ret_possible
        .iter()
        .map(|i| {
            let variant = messages
                .iter()
                .find(|x| &x.ident == *i)
                .expect("variant ident found");
            variant
        })
        .collect::<Vec<_>>();

    let (ret_name, special_type, ret_matches) = if need_enum {
        let ret_name = quote::format_ident!("{}Ret", v.ident);

        let ret_matches = ret_variants
            .iter()
            .map(|variant| {
                let ident = &variant.ident;
                let names = iterator_names(&mut variant.fields.iter(), "param");
                let params = if names.is_empty() {
                    quote! {}
                } else {
                    quote! { ( #(#names),* ) }
                };
                quote! { #impl_name :: #ident #params => Some(#ret_name :: #ident #params), }
            })
            .collect::<Vec<_>>();

        let rev_ret_matches = ret_variants
            .iter()
            .map(|variant| {
                let ident = &variant.ident;
                let names = iterator_names(&mut variant.fields.iter(), "param");
                let params = if names.is_empty() {
                    quote! {}
                } else {
                    quote! { ( #(#names),* ) }
                };
                quote! { #ret_name :: #ident #params => #impl_name :: #ident #params, }
            })
            .collect::<Vec<_>>();

        (
            quote! { #ret_name },
            quote! {
                pub enum #ret_name {
                    #(#ret_variants),*
                }

                impl From<#ret_name> for #impl_name {
                    fn from(r: #ret_name) -> #impl_name {
                        match r {
                            #(#rev_ret_matches)*
                        }
                    }
                }
            },
            ret_matches,
        )
    } else {
        let variant = &ret_variants[0];
        let fields = &variant.fields.iter().collect::<Vec<_>>();
        let ident = &variant.ident;
        let names = iterator_names(&mut variant.fields.iter(), "param");
        let params = if names.is_empty() {
            quote! {}
        } else {
            quote! { ( #(#names),* ) }
        };
        let params_ret = if names.is_empty() {
            quote! { () }
        } else if names.len() == 1 {
            let names0 = &names[0];
            quote! { #names0 }
        } else {
            quote! { ( #(#names),* ) }
        };
        let ret_matches = vec![quote! { #impl_name :: #ident #params => Some(#params_ret), }];
        //ret_possible[0].
        (quote! { #(#fields),* }, quote! {}, ret_matches)
    };

    Some(quote! {
        #special_type
        pub fn #fn_name(message: #impl_name) -> Option<#ret_name> {
            match message {
                #(#ret_matches)*
                _ => None,
            }
        }
    })
}

fn server_msg_generate(
    context: &Context,
    st: &Ident,
    messages: &[&syn::Variant],
) -> Option<proc_macro2::TokenStream> {
    let impl_name = &context.msg_name;
    let fn_name = quote::format_ident!("server_{}_message_filter", camel_to_snake(&st.to_string()));

    if messages.len() == 0 {
        return None;
    }

    let need_enum = messages.len() > 1;

    let (ret_name, ret_definition) = if need_enum {
        let ret_name = quote::format_ident!("On{}Msg", st);

        let ret_variants = messages
            .iter()
            .map(|v| {
                let id = &v.ident;
                let params = if v.fields.is_empty() {
                    quote! {}
                } else {
                    let field_types = v.fields.iter().map(|field| &field.ty).collect::<Vec<_>>();
                    quote! { ( #(#field_types),* ) }
                };
                quote! { #id #params }
            })
            .collect::<Vec<_>>();
        (
            quote! { #ret_name },
            quote! {
                pub enum #ret_name {
                    #(#ret_variants),*
                }
            },
        )
    } else {
        let m = &messages[0];
        let ty = if m.fields.is_empty() {
            quote! { () }
        } else if m.fields.len() == 1 {
            let ty = &m.fields.iter().next().expect("type next exist").ty;
            quote! { #ty }
        } else {
            let ty_params = m.fields.iter().map(|f| &f.ty).collect::<Vec<_>>();
            quote! { ( #(#ty_params),* ) }
        };
        (ty, quote! {})
    };

    let fn_matches = messages
        .iter()
        .map(|variant| {
            let id = &variant.ident;
            let names = iterator_names(&mut variant.fields.iter(), "param");

            let names_commas = quote! { ( #(#names),* ) };
            let (params, params_no_enum) = if names.is_empty() {
                (quote! {}, quote! { () })
            } else if names.len() == 1 {
                (names_commas.clone(), quote! { #(#names),* })
            } else {
                (names_commas.clone(), names_commas)
            };

            if need_enum {
                quote! { #impl_name :: #id #params => { Some(#ret_name :: #id #params) } }
            } else {
                quote! { #impl_name :: #id #params => { Some(#params_no_enum) } }
            }
        })
        .collect::<Vec<_>>();

    Some(quote! {
        #ret_definition

        pub fn #fn_name(message: #impl_name) -> Option<#ret_name> {
            match message {
                #(#fn_matches)*
                _ => None,
            }
        }
    })
}

/// Context of this macro which contains the type name of the message and the state
struct Context {
    /// Ident of the state type
    state_name: Ident,
    /// Ident of the message type
    msg_name: Ident,
    /// Transitions
    transitions: Vec<Transition>,
}

impl Context {
    pub fn transitions_for_message<'a>(
        &'a self,
        m: &'a Ident,
    ) -> impl Iterator<Item = &'a Transition> {
        self.transitions
            .iter()
            .filter(move |transition| &transition.message == m)
    }

    pub fn transitions_messages_starts_with_state<'a>(
        &'a self,
        starts_with: &'a Ident,
    ) -> impl Iterator<Item = &'a Ident> {
        self.transitions.iter().filter_map(move |transition| {
            if &transition.start == starts_with {
                Some(&transition.message)
            } else {
                None
            }
        })
    }
}

/// Transition defined as : start + message = end
struct Transition {
    /// Start state
    start: Ident,
    /// Message for transition
    message: Ident,
    /// End state
    end: Ident,
}

impl std::fmt::Debug for Transition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} + {} = {}", self.start, self.message, self.end)
    }
}

fn parse_transitions(group: proc_macro2::TokenStream) -> Vec<Transition> {
    let mut transitions = Vec::new();
    let mut it = group.into_iter();
    while let Some(x) = it.next() {
        let state_begin = get_ident(x).unwrap();
        is_punct(it.next().unwrap(), '+', Spacing::Alone).unwrap();
        let trans_msg = get_ident(it.next().unwrap()).unwrap();

        is_punct(it.next().unwrap(), '=', Spacing::Alone).unwrap();
        let state_end = get_ident(it.next().unwrap()).unwrap();

        transitions.push(Transition {
            start: state_begin,
            message: trans_msg,
            end: state_end,
        });

        if let Some(x) = it.next() {
            is_punct(x, ',', Spacing::Alone).unwrap()
        }
    }
    transitions
}

fn get_ident(tt: TokenTree) -> Result<proc_macro2::Ident, String> {
    match tt {
        TokenTree::Ident(ident) => Ok(ident),
        TokenTree::Group(group) => Err(format!(
            "expecting ident but got group at {:?}",
            group.span()
        )),
        TokenTree::Punct(punct) => Err(format!(
            "expecting ident but got punct at {:?}",
            punct.span()
        )),
        TokenTree::Literal(literal) => Err(format!(
            "expecting ident but got literal {:?} at {:?}",
            literal,
            literal.span()
        )),
    }
}

fn is_punct(tt: TokenTree, c: char, spacing: Spacing) -> Result<(), String> {
    match tt {
        TokenTree::Punct(punct) => {
            if punct.as_char() == c && punct.spacing() == spacing {
                Ok(())
            } else {
                Err(format!(
                    "expecting {}:{:?} but got {}:{:?}",
                    c,
                    spacing,
                    punct.as_char(),
                    punct.spacing()
                ))
            }
        }
        TokenTree::Group(group) => Err(format!(
            "expecting punct but got group at {:?}",
            group.span()
        )),
        TokenTree::Ident(ident) => Err(format!("expecting punct but got ident {}", ident)),
        TokenTree::Literal(literal) => Err(format!("expecting punct but got literal {}", literal)),
    }
}

fn get_group(
    tt: TokenTree,
    delimiter: proc_macro2::Delimiter,
) -> Result<proc_macro2::TokenStream, String> {
    match tt {
        TokenTree::Group(group) => {
            if group.delimiter() == delimiter {
                Ok(group.stream())
            } else {
                Err(format!("wrong delimiter for group"))
            }
        }
        TokenTree::Ident(ident) => Err(format!("expecting group but got ident {}", ident)),
        TokenTree::Punct(punct) => Err(format!("expecting group but got punct {}", punct)),
        TokenTree::Literal(literal) => Err(format!("expecting group but got literal {}", literal)),
    }
}

fn camel_to_snake(s: &str) -> String {
    let mut snake_case = String::new();

    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                snake_case.push('_'); // Add underscore before uppercase letters (except at start)
            }
            snake_case.push(c.to_ascii_lowercase());
        } else {
            snake_case.push(c);
        }
    }

    snake_case
}

fn iterator_names<I: Iterator>(iter: &mut I, prefix: &str) -> Vec<Ident> {
    iter.enumerate()
        .map(|(i, _)| quote::format_ident!("{}{}", prefix, i))
        .collect()
}
