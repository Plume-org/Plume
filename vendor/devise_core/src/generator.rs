use syn;
use proc_macro::{TokenStream, Diagnostic};
use proc_macro2::TokenStream as TokenStream2;

use spanned::Spanned;
use ext::GenericExt;

use field::{Field, Fields};
use support::{GenericSupport, DataSupport};
use derived::{Derived, Variant, Struct, Enum};

pub type Result<T> = ::std::result::Result<T, Diagnostic>;
pub type MapResult = Result<TokenStream2>;

macro_rules! validator {
    ($fn_name:ident: $validate_fn_type:ty, $field:ident) => {
        pub fn $fn_name<F: 'static>(&mut self, f: F) -> &mut Self
            where F: Fn(&DeriveGenerator, $validate_fn_type) -> Result<()>
        {
            self.$field = Box::new(f);
            self
        }
    }
}

macro_rules! mappers {
    ($(($map_f:ident, $try_f:ident, $get_f:ident, $default_f:ident): $type:ty, $vec:ident),*) => (
        pub(crate) fn push_default_mappers(&mut self) {
            $(self.$vec.push(Box::new($default_f));)*
        }

        $(
            pub fn $map_f<F: 'static>(&mut self, f: F) -> &mut Self
                where F: Fn(&DeriveGenerator, $type) -> TokenStream2
            {
                if !self.$vec.is_empty() {
                    let last = self.$vec.len() - 1;
                    self.$vec[last] = Box::new(move |g, v| Ok(f(g, v)));
                }

                self
            }

            pub fn $try_f<F: 'static>(&mut self, f: F) -> &mut Self
                where F: Fn(&DeriveGenerator, $type) -> MapResult
            {
                if !self.$vec.is_empty() {
                    let last = self.$vec.len() - 1;
                    self.$vec[last] = Box::new(f);
                }

                self
            }

            pub fn $get_f(&self) -> &Box<dyn Fn(&DeriveGenerator, $type) -> MapResult> {
                assert!(!self.$vec.is_empty());
                let last = self.$vec.len() - 1;
                &self.$vec[last]
            }
        )*
    )
}

// FIXME: Take a `Box<Fn>` everywhere so we can capture args!
pub struct DeriveGenerator {
    pub input: syn::DeriveInput,
    pub trait_impl: syn::ItemImpl,
    pub trait_path: syn::Path,
    pub(crate) generic_support: GenericSupport,
    pub(crate) data_support: DataSupport,
    pub(crate) enum_validator: Box<dyn Fn(&DeriveGenerator, Enum) -> Result<()>>,
    pub(crate) struct_validator: Box<dyn Fn(&DeriveGenerator, Struct) -> Result<()>>,
    pub(crate) generics_validator: Box<dyn Fn(&DeriveGenerator, &::syn::Generics) -> Result<()>>,
    pub(crate) fields_validator: Box<dyn Fn(&DeriveGenerator, Fields) -> Result<()>>,
    pub(crate) type_generic_mapper: Option<Box<dyn Fn(&DeriveGenerator, &syn::Ident, &syn::TypeParam) -> TokenStream2>>,
    pub(crate) generic_replacements: Vec<(usize, usize)>,
    pub(crate) functions: Vec<Box<dyn Fn(&DeriveGenerator, TokenStream2) -> TokenStream2>>,
    pub(crate) enum_mappers: Vec<Box<dyn Fn(&DeriveGenerator, Enum) -> MapResult>>,
    pub(crate) struct_mappers: Vec<Box<dyn Fn(&DeriveGenerator, Struct) -> MapResult>>,
    pub(crate) variant_mappers: Vec<Box<dyn Fn(&DeriveGenerator, Variant) -> MapResult>>,
    pub(crate) fields_mappers: Vec<Box<dyn Fn(&DeriveGenerator, Fields) -> MapResult>>,
    pub(crate) field_mappers: Vec<Box<dyn Fn(&DeriveGenerator, Field) -> MapResult>>,
}

pub fn default_enum_mapper(gen: &DeriveGenerator, data: Enum) -> MapResult {
    let variant = data.variants().map(|v| &v.value.ident);
    let fields = data.variants().map(|v| v.fields().match_tokens());
    let enum_name = ::std::iter::repeat(&data.derive_input.ident);
    let expression = data.variants()
        .map(|v| gen.variant_mapper()(gen, v))
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        // FIXME: Check if we can also use id_match_tokens due to match
        // ergonomics. I don't think so, though. If we can't, then ask (in
        // `function`) whether receiver is `&self`, `&mut self` or `self` and
        // bind match accordingly.
        match self {
            #(#enum_name::#variant #fields => { #expression }),*
        }
    })
}

pub fn null_enum_mapper(gen: &DeriveGenerator, data: Enum) -> MapResult {
    let expression = data.variants()
        .map(|v| gen.variant_mapper()(gen, v))
        .collect::<Result<Vec<_>>>()?;

    Ok(quote!(#(#expression)*))
}

pub fn default_struct_mapper(gen: &DeriveGenerator, data: Struct) -> MapResult {
    gen.fields_mapper()(gen, data.fields())
}

pub fn default_variant_mapper(gen: &DeriveGenerator, data: Variant) -> MapResult {
    gen.fields_mapper()(gen, data.fields())
}

pub fn default_field_mapper(_gen: &DeriveGenerator, _data: Field) -> MapResult {
    Ok(TokenStream2::new())
}

pub fn default_fields_mapper(g: &DeriveGenerator, fields: Fields) -> MapResult {
    let field = fields.iter()
        .map(|field| g.field_mapper()(g, field))
        .collect::<Result<Vec<_>>>()?;

    Ok(quote!({ #(#field)* }))
}

impl DeriveGenerator {
    pub fn build_for(input: TokenStream, trait_impl: TokenStream2) -> DeriveGenerator {
        let trait_impl: syn::ItemImpl = syn::parse2(quote!(#trait_impl for Foo {}))
            .expect("invalid impl");
        let trait_path = trait_impl.trait_.clone().expect("impl does not have trait").1;
        let input = syn::parse(input).expect("invalid derive input");

        DeriveGenerator {
            input, trait_impl, trait_path,
            generic_support: GenericSupport::None,
            data_support: DataSupport::None,
            type_generic_mapper: None,
            generic_replacements: vec![],
            enum_validator: Box::new(|_, _| Ok(())),
            struct_validator: Box::new(|_, _| Ok(())),
            generics_validator: Box::new(|_, _| Ok(())),
            fields_validator: Box::new(|_, _| Ok(())),
            functions: vec![],
            enum_mappers: vec![],
            struct_mappers: vec![],
            variant_mappers: vec![],
            field_mappers: vec![],
            fields_mappers: vec![],
        }
    }

    pub fn generic_support(&mut self, support: GenericSupport) -> &mut Self {
        self.generic_support = support;
        self
    }

    pub fn data_support(&mut self, support: DataSupport) -> &mut Self {
        self.data_support = support;
        self
    }

    pub fn map_type_generic<F: 'static>(&mut self, f: F) -> &mut Self
        where F: Fn(&DeriveGenerator, &syn::Ident, &syn::TypeParam) -> TokenStream2
    {
        self.type_generic_mapper = Some(Box::new(f));
        self
    }

    pub fn replace_generic(&mut self, trait_gen: usize, impl_gen: usize) -> &mut Self {
        self.generic_replacements.push((trait_gen, impl_gen));
        self
    }

    validator!(validate_enum: Enum, enum_validator);
    validator!(validate_struct: Struct, struct_validator);
    validator!(validate_generics: &syn::Generics, generics_validator);
    validator!(validate_fields: Fields, fields_validator);

    pub fn function<F: 'static>(&mut self, f: F) -> &mut Self 
        where F: Fn(&DeriveGenerator, TokenStream2) -> TokenStream2
    {
        self.functions.push(Box::new(f));
        self.push_default_mappers();
        self
    }

    mappers! {
        (map_struct, try_map_struct, struct_mapper, default_struct_mapper): Struct, struct_mappers,
        (map_enum, try_map_enum, enum_mapper, default_enum_mapper): Enum, enum_mappers,
        (map_variant, try_map_variant, variant_mapper, default_variant_mapper): Variant, variant_mappers,
        (map_fields, try_map_fields, fields_mapper, default_fields_mapper): Fields, fields_mappers,
        (map_field, try_map_field, field_mapper, default_field_mapper): Field, field_mappers
    }

    fn _to_tokens(&mut self) -> Result<TokenStream> {
        use syn::*;

        // Step 1: Run all validators.
        // Step 1a: First, check for data support.
        let (span, support) = (self.input.span(), self.data_support);
        match self.input.data {
            Data::Struct(ref data) => {
                let named = Struct::from(&self.input, data).fields().are_named();
                if named && !support.contains(DataSupport::NamedStruct) {
                    return Err(span.error("named structs are not supported"));
                }

                if !named && !support.contains(DataSupport::TupleStruct) {
                    return Err(span.error("tuple structs are not supported"));
                }
            }
            Data::Enum(..) if !support.contains(DataSupport::Enum) => {
                return Err(span.error("enums are not supported"));
            }
            Data::Union(..) if !support.contains(DataSupport::Union) => {
                return Err(span.error("unions are not supported"));
            }
            _ => { /* we're okay! */ }
        }

        // Step 1b: Second, check for generics support.
        for generic in &self.input.generics.params {
            use syn::GenericParam::*;

            let (span, support) = (generic.span(), self.generic_support);
            match generic {
                Type(..) if !support.contains(GenericSupport::Type) => {
                    return Err(span.error("type generics are not supported"));
                }
                Lifetime(..) if !support.contains(GenericSupport::Lifetime) => {
                    return Err(span.error("lifetime generics are not supported"));
                }
                Const(..) if !support.contains(GenericSupport::Const) => {
                    return Err(span.error("const generics are not supported"));
                }
                _ => { /* we're okay! */ }
            }
        }

        // Step 1c: Third, run the custom validators.
        (self.generics_validator)(self, &self.input.generics)?;
        match self.input.data {
            Data::Struct(ref data) => {
                let derived = Derived::from(&self.input, data);
                (self.struct_validator)(self, derived)?;
                (self.fields_validator)(self, derived.fields())?;
            }
            Data::Enum(ref data) => {
                let derived = Derived::from(&self.input, data);
                (self.enum_validator)(self, derived)?;
                for variant in derived.variants() {
                    (self.fields_validator)(self, variant.fields())?;
                }
            }
            Data::Union(ref _data) => unimplemented!("union custom validation"),
        }

        // Step 2: Generate the code!
        // Step 2a: Generate the code for each function.
        let mut function_code = vec![];
        for i in 0..self.functions.len() {
            let function = &self.functions[i];
            let inner = match self.input.data {
                Data::Struct(ref data) => {
                    let derived = Derived::from(&self.input, data);
                    self.struct_mappers[i](self, derived)?
                }
                Data::Enum(ref data) => {
                    let derived = Derived::from(&self.input, data);
                    self.enum_mappers[i](self, derived)?
                }
                Data::Union(ref _data) => unimplemented!("can't gen unions yet"),
            };

            function_code.push(function(self, inner));
        }

        // Step 2b: Create a couple of generics to mutate with user's input.
        let mut generics = self.input.generics.clone();

        // Step 2c: Add additional where bounds if the generator asks for it.
        if let Some(ref type_mapper) = self.type_generic_mapper {
            for ty in self.input.generics.type_params() {
                let new_ty = type_mapper(self, &ty.ident, ty);
                let clause = syn::parse2(new_ty).expect("invalid type generic mapping");
                generics.make_where_clause().predicates.push(clause);
            }
        }

        // Step 2d: Add any generics in the trait.
        let mut generics_for_impl_generics = generics.clone();
        for (i, trait_param) in self.trait_impl.generics.params.iter().enumerate() {
            // Step 2d.0: Perform a generic replacement if requested. Here,
            // we determine if a generic (i) in the trait is going to replace a
            // generic in the user's type (the `jth` of the right kind).
            let replacement = self.generic_replacements.iter()
                .filter(|r| r.0 == i)
                .next();

            if let Some((_, j)) = replacement {
                use syn::{punctuated::Punctuated, token::Comma};

                // Step 2d.1: Actually perform the replacement.
                let replace_in = |ps: &mut Punctuated<GenericParam, Comma>| -> bool {
                    ps.iter_mut()
                        .filter(|param| param.kind() == trait_param.kind())
                        .nth(*j)
                        .map(|impl_param| *impl_param = trait_param.clone())
                        .is_some()
                };

                // Step 2d.2: If it fails, insert a new impl generic.
                // NOTE: It's critical that `generics` is attempted first!
                // Otherwise, we might replace generics that don't exist in the
                // user's type.
                if !replace_in(&mut generics.params)
                    || !replace_in(&mut generics_for_impl_generics.params)
                {
                    generics_for_impl_generics.params.insert(0, trait_param.clone());
                }
            } else {
                // Step 2d.2: Otherwise, insert a new impl<..> generic.
                generics_for_impl_generics.params.insert(0, trait_param.clone());
            }
        }

        // Step 2e: Split the generics, but use the `impl_generics` from above.
        let (impl_gen, _, _) = generics_for_impl_generics.split_for_impl();
        let (_, ty_gen, where_gen) = generics.split_for_impl();

        // Step 2b: Generate the complete implementation.
        let target = &self.input.ident;
        let trait_name = &self.trait_path;
        Ok(quote! {
            impl #impl_gen #trait_name for #target #ty_gen #where_gen {
                #(#function_code)*
            }
        }.into())
    }

    pub fn debug(&mut self) -> &mut Self {
        match self._to_tokens() {
            Ok(tokens) => println!("Tokens produced: {}", tokens.to_string()),
            Err(e) => println!("Error produced: {:?}", e)
        }

        self
    }

    pub fn to_tokens(&mut self) -> TokenStream {
        // FIXME: Emit something like: Trait: msg.
        self._to_tokens()
            .unwrap_or_else(|diag| {
                if let Some(last) = self.trait_path.segments.last() {
                    use proc_macro::Span;
                    use proc_macro::Level::*;

                    let id = &last.value().ident;
                    let msg = match diag.level() {
                        Error => format!("error occurred while deriving `{}`", id),
                        Warning => format!("warning issued by `{}` derive", id),
                        Note => format!("note issued by `{}` derive", id),
                        Help => format!("help provided by `{}` derive", id),
                        _ => format!("while deriving `{}`", id)
                    };

                    diag.span_note(Span::call_site(), msg).emit();
                }

                TokenStream::new().into()
            })
    }
}
