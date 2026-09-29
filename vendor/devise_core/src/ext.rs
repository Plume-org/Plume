use syn::{*, punctuated::Punctuated, token::Comma};

pub trait PathExt {
    fn is(&self, global: bool, segments: &[&str]) -> bool;
    fn is_local(&self, segments: &[&str]) -> bool;
    fn is_global(&self, segments: &[&str]) -> bool;
    fn generics(&self) -> Option<&Punctuated<GenericArgument, Comma>>;
}

pub trait TypeExt {
    fn strip_lifetimes(&mut self);
    fn with_stripped_lifetimes(&self) -> Type;
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum GenericKind { Lifetime, Type, Binding, Const, Constraint }

pub trait GenericExt {
    fn kind(&self) -> GenericKind;
}

pub trait Split2<A, B>: Sized + Iterator {
    fn split2(self) -> (Vec<A>, Vec<B>);
}

pub trait Split3<A, B, C>: Sized + Iterator {
    fn split3(self) -> (Vec<A>, Vec<B>, Vec<C>);
}

impl PathExt for Path {
    fn is(&self, global: bool, segments: &[&str]) -> bool {
        if self.leading_colon.is_some() != global || self.segments.len() != segments.len() {
            return false;
        }

        for (segment, wanted) in self.segments.iter().zip(segments.iter()) {
            if segment.ident != wanted {
                return false;
            }
        }

        true
    }

    fn is_local(&self, segments: &[&str]) -> bool {
        self.is(false, segments)
    }

    fn is_global(&self, segments: &[&str]) -> bool {
        self.is(true, segments)
    }

    fn generics(&self) -> Option<&Punctuated<GenericArgument, Comma>> {
        self.segments.last().and_then(|last| {
            match last.value().arguments {
                PathArguments::AngleBracketed(ref args) => Some(&args.args),
                _ => None
            }
        })
    }
}

impl<A, B, I: IntoIterator<Item = (A, B)> + Iterator> Split2<A, B> for I {
    fn split2(self) -> (Vec<A>, Vec<B>) {
        let (mut first, mut second) = (vec![], vec![]);
        self.into_iter().for_each(|(a, b)| {
            first.push(a);
            second.push(b);
        });

        (first, second)
    }
}

impl<A, B, C, I: IntoIterator<Item = (A, B, C)> + Iterator> Split3<A, B, C> for I {
    fn split3(self) -> (Vec<A>, Vec<B>, Vec<C>) {
        let (mut first, mut second, mut third) = (vec![], vec![], vec![]);
        self.into_iter().for_each(|(a, b, c)| {
            first.push(a);
            second.push(b);
            third.push(c);
        });

        (first, second, third)
    }
}

impl TypeExt for Type {
    fn strip_lifetimes(&mut self) {
        strip(self);
    }

    fn with_stripped_lifetimes(&self) -> Type {
        let mut new = self.clone();
        new.strip_lifetimes();
        new
    }
}

fn make_wild(lifetime: &mut Lifetime) {
    lifetime.ident = Ident::new("_", lifetime.ident.span());
}

fn strip(ty: &mut Type) {
    match *ty {
        Type::Reference(ref mut inner) => {
            inner.lifetime.as_mut().map(make_wild);
            strip(&mut inner.elem);
        }
        Type::Slice(ref mut inner) => strip(&mut inner.elem),
        Type::Array(ref mut inner) => strip(&mut inner.elem),
        Type::Ptr(ref mut inner) => strip(&mut inner.elem),
        Type::Paren(ref mut inner) => strip(&mut inner.elem),
        Type::Group(ref mut inner) => strip(&mut inner.elem),
        Type::BareFn(ref mut inner) => {
            inner.lifetimes.as_mut().map(strip_bound_lifetimes);
            if let ReturnType::Type(_, ref mut ty) = inner.output {
                strip(ty);
            }

            inner.inputs.iter_mut().for_each(|input| strip(&mut input.ty));
        }
        Type::Tuple(ref mut inner) => {
            inner.elems.iter_mut().for_each(strip);
        }
        Type::Path(ref mut inner) => {
            if let Some(ref mut qself) = inner.qself {
                strip(&mut qself.ty);
            }

            strip_path(&mut inner.path);
        }
        Type::ImplTrait(ref mut inner) => strip_bounds(&mut inner.bounds),
        Type::TraitObject(ref mut inner) => strip_bounds(&mut inner.bounds),
        Type::Infer(_) | Type::Macro(_) | Type::Verbatim(_) | Type::Never(_) => {  }
    }
}

fn strip_bound_lifetimes(bound: &mut BoundLifetimes) {
    bound.lifetimes.iter_mut().for_each(|d| make_wild(&mut d.lifetime));
}

fn strip_path(path: &mut Path) {
    for segment in path.segments.iter_mut() {
        use syn::GenericArgument::*;

        match segment.arguments {
            PathArguments::AngleBracketed(ref mut inner) => {
                let args = inner.args.clone();
                inner.args = args.into_pairs().filter_map(|mut pair| {
                    match pair.value_mut() {
                        Lifetime(ref mut l) => make_wild(l),
                        Type(ref mut ty) => strip(ty),
                        Binding(ref mut inner) => strip(&mut inner.ty),
                        Constraint(ref mut inner) => strip_bounds(&mut inner.bounds),
                        Const(..) => { /* ? */ }
                    }

                    Some(pair)
                }).collect();
            }
            PathArguments::Parenthesized(ref mut args) => {
                args.inputs.iter_mut().for_each(strip);
                if let ReturnType::Type(_, ref mut ty) = args.output {
                    strip(ty);
                }
            }
            PathArguments::None => {  }
        }
    }
}

fn strip_bounds(bounds: &mut Punctuated<TypeParamBound, token::Add>) {
    let old_bounds = bounds.clone();
    *bounds = old_bounds.into_pairs().filter_map(|mut pair| {
        match pair.value_mut() {
            TypeParamBound::Lifetime(ref mut l) => make_wild(l),
            TypeParamBound::Trait(ref mut inner) => {
                inner.lifetimes.as_mut().map(strip_bound_lifetimes);
                strip_path(&mut inner.path);
            }
        }

        Some(pair)
    }).collect();
}

impl GenericExt for GenericArgument {
    fn kind(&self) -> GenericKind {
        match *self {
            GenericArgument::Lifetime(..) => GenericKind::Lifetime,
            GenericArgument::Type(..) => GenericKind::Type,
            GenericArgument::Binding(..) => GenericKind::Binding,
            GenericArgument::Constraint(..) => GenericKind::Constraint,
            GenericArgument::Const(..) => GenericKind::Const,
        }
    }
}

impl GenericExt for GenericParam {
    fn kind(&self) -> GenericKind {
        match *self {
            GenericParam::Lifetime(..) => GenericKind::Lifetime,
            GenericParam::Type(..) => GenericKind::Type,
            GenericParam::Const(..) => GenericKind::Const,
        }
    }
}
