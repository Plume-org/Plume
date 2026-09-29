mod meta_item;

use syn::{self, Lit::*};

pub use self::meta_item::{MetaItem, MetaItemList};

use generator::Result;
use spanned::Spanned;
use proc_macro::Span;

// Spans of k/v pair, key, then value.
#[derive(Copy, Clone)]
pub struct SpanWrapped<T> {
    pub span: Span,
    pub full_span: Span,
    pub value: T,
}

pub trait FromMeta: Sized {
    fn from_meta(meta: MetaItem) -> Result<Self>;

    fn from_attr(name: &str, attr: &syn::Attribute) -> Result<Self> {
        let meta = attr.interpret_meta().ok_or_else(|| {
            attr.span()
                .error("malformed attribute")
                .help(format!("expected syntax: #[{}(key = value, ..)]", name))
        })?;

        if let syn::Meta::List(list) = meta {
            let list = MetaItemList { ident: &list.ident, iter: &list.nested };
            Self::from_meta(MetaItem::List(list))
        } else {
            Err(meta.span()
                    .error("malformed attribute: expected list")
                    .help(format!("expected syntax: #[{}(key = value, ..)]", name)))
        }
    }

    fn from_attrs(name: &str, attrs: &[syn::Attribute]) -> Option<Result<Self>> {
        let tokens = name.parse()
            .expect(&format!("`{}` contained invalid tokens", name));

        let path = syn::parse(tokens)
            .expect(&format!("`{}` was not a valid path", name));

        let mut matches = attrs.iter().filter(|attr| attr.path == path);
        let attr = matches.next()?;

        if let Some(extra) = matches.next() {
            let msg = format!("duplicate invocation of `{}` attribute", name);
            return Some(Err(extra.span().error(msg)));
        }

        Some(Self::from_attr(name, attr))
    }

    fn default() -> Option<Self> {
        None
    }
}

impl FromMeta for isize {
    fn from_meta(meta: MetaItem) -> Result<Self> {
        if let Int(i) = meta.lit()? {
            if i.value() <= isize::max_value() as u64 {
                return Ok(i.value() as isize);
            }

            return Err(meta.value_span().error("value is out of range for `isize`"));
        }

        Err(meta.value_span().error("invalid value: expected integer literal"))
    }
}

impl FromMeta for usize {
    fn from_meta(meta: MetaItem) -> Result<Self> {
        if let Int(i) = meta.lit()? {
            if i.value() <= usize::max_value() as u64 {
                return Ok(i.value() as usize);
            }

            return Err(meta.value_span().error("value is out of range for `usize`"));
        }

        Err(meta.value_span().error("invalid value: expected unsigned integer literal"))
    }
}

impl FromMeta for String {
    fn from_meta(meta: MetaItem) -> Result<Self> {
        if let Str(s) = meta.lit()? {
            return Ok(s.value());
        }

        Err(meta.value_span().error("invalid value: expected string literal"))
    }
}

impl FromMeta for bool {
    fn from_meta(meta: MetaItem) -> Result<Self> {
        if let MetaItem::Ident(_) = meta {
            return Ok(true);
        }

        if let Bool(b) = meta.lit()? {
            return Ok(b.value);
        }

        return Err(meta.value_span().error("invalid value: expected boolean"));
    }
}

impl<T: FromMeta> FromMeta for Option<T> {
    fn from_meta(meta: MetaItem) -> Result<Self> {
        T::from_meta(meta).map(Some)
    }

    fn default() -> Option<Self> {
        Some(None)
    }
}

impl<T: FromMeta> FromMeta for SpanWrapped<T> {
    fn from_meta(meta: MetaItem) -> Result<Self> {
        let span = meta.value_span();
        let full_span = meta.span();
        T::from_meta(meta).map(|value| SpanWrapped { full_span, span, value })
    }
}

impl<T: ::quote::ToTokens> ::quote::ToTokens for SpanWrapped<T> {
    fn to_tokens(&self, tokens: &mut ::proc_macro2::TokenStream) {
        self.value.to_tokens(tokens)
    }
}

use std::ops::Deref;

impl<T> Deref for SpanWrapped<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

use std::fmt;

impl<T: fmt::Debug> fmt::Debug for SpanWrapped<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("SpanWrapped")
            .field(&self.value)
            .finish()
    }
}
