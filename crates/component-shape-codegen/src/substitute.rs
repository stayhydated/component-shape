use super::*;

use syn::visit_mut::{self, VisitMut as _};

/// Substitute a field type for every `_` occurrence inside a type.
pub fn substitute_infer_in_type(ty: &Type, replacement: &Type) -> Type {
    match ty {
        Type::Infer(_) => replacement.clone(),
        Type::Path(type_path) => {
            let mut type_path = type_path.clone();
            type_path.path = substitute_infer_in_path(&type_path.path, replacement);
            Type::Path(type_path)
        },
        Type::Array(array) => {
            let mut array = array.clone();
            array.elem = Box::new(substitute_infer_in_type(&array.elem, replacement));
            Type::Array(array)
        },
        Type::Slice(slice) => {
            let mut slice = slice.clone();
            slice.elem = Box::new(substitute_infer_in_type(&slice.elem, replacement));
            Type::Slice(slice)
        },
        Type::Ptr(ptr) => {
            let mut ptr = ptr.clone();
            ptr.elem = Box::new(substitute_infer_in_type(&ptr.elem, replacement));
            Type::Ptr(ptr)
        },
        Type::BareFn(bare_fn) => {
            let mut bare_fn = bare_fn.clone();
            for input in &mut bare_fn.inputs {
                input.ty = substitute_infer_in_type(&input.ty, replacement);
            }
            substitute_infer_in_return_type(&mut bare_fn.output, replacement);
            Type::BareFn(bare_fn)
        },
        Type::TraitObject(trait_object) => {
            let mut trait_object = trait_object.clone();
            substitute_infer_in_bounds(&mut trait_object.bounds, replacement);
            Type::TraitObject(trait_object)
        },
        Type::ImplTrait(impl_trait) => {
            let mut impl_trait = impl_trait.clone();
            substitute_infer_in_bounds(&mut impl_trait.bounds, replacement);
            Type::ImplTrait(impl_trait)
        },
        Type::Tuple(tuple) => {
            let mut tuple = tuple.clone();
            tuple.elems = tuple
                .elems
                .iter()
                .map(|ty| substitute_infer_in_type(ty, replacement))
                .collect();
            Type::Tuple(tuple)
        },
        Type::Paren(paren) => {
            let mut paren = paren.clone();
            paren.elem = Box::new(substitute_infer_in_type(&paren.elem, replacement));
            Type::Paren(paren)
        },
        Type::Group(group) => {
            let mut group = group.clone();
            group.elem = Box::new(substitute_infer_in_type(&group.elem, replacement));
            Type::Group(group)
        },
        Type::Reference(reference) => {
            let mut reference = reference.clone();
            *reference.elem = substitute_infer_in_type(&reference.elem, replacement);
            Type::Reference(reference)
        },
        _ => ty.clone(),
    }
}

/// Substitute a field type for every `_` occurrence inside an expression.
///
/// This is primarily useful for configured component shape expressions such as
/// `crate::Select::<_>::searchable(true)`, where the expression must retain
/// expression-position turbofish syntax while its base shape metadata uses a
/// type-position path.
pub fn substitute_infer_in_expr(expr: &Expr, replacement: &Type) -> Expr {
    let mut expr = expr.clone();
    InferSubstitutor { replacement }.visit_expr_mut(&mut expr);
    expr
}

struct InferSubstitutor<'a> {
    replacement: &'a Type,
}

impl visit_mut::VisitMut for InferSubstitutor<'_> {
    fn visit_type_mut(&mut self, node: &mut Type) {
        *node = substitute_infer_in_type(node, self.replacement);
    }

    fn visit_path_mut(&mut self, node: &mut Path) {
        *node = substitute_infer_in_path(node, self.replacement);
    }
}

fn substitute_infer_in_return_type(return_type: &mut syn::ReturnType, replacement: &Type) {
    if let syn::ReturnType::Type(_, ty) = return_type {
        **ty = substitute_infer_in_type(ty, replacement);
    }
}

fn substitute_infer_in_bounds(
    bounds: &mut syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
    replacement: &Type,
) {
    for bound in bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            trait_bound.path = substitute_infer_in_path(&trait_bound.path, replacement);
        }
    }
}

/// Substitute a field type for every `_` occurrence inside path arguments.
pub fn substitute_infer_in_path(path: &Path, replacement: &Type) -> Path {
    let mut path = path.clone();

    for segment in &mut path.segments {
        substitute_infer_in_path_arguments(&mut segment.arguments, replacement);
    }

    path
}

fn substitute_infer_in_path_arguments(arguments: &mut syn::PathArguments, replacement: &Type) {
    match arguments {
        syn::PathArguments::AngleBracketed(args) => {
            substitute_infer_in_angle_bracketed_arguments(args, replacement);
        },
        syn::PathArguments::Parenthesized(args) => {
            args.inputs = args
                .inputs
                .iter()
                .map(|ty| substitute_infer_in_type(ty, replacement))
                .collect();
            substitute_infer_in_return_type(&mut args.output, replacement);
        },
        syn::PathArguments::None => {},
    }
}

fn substitute_infer_in_angle_bracketed_arguments(
    args: &mut syn::AngleBracketedGenericArguments,
    replacement: &Type,
) {
    for arg in &mut args.args {
        match arg {
            syn::GenericArgument::Type(ty) => {
                *ty = substitute_infer_in_type(ty, replacement);
            },
            syn::GenericArgument::AssocType(assoc_type) => {
                if let Some(generics) = &mut assoc_type.generics {
                    substitute_infer_in_angle_bracketed_arguments(generics, replacement);
                }
                assoc_type.ty = substitute_infer_in_type(&assoc_type.ty, replacement);
            },
            syn::GenericArgument::Constraint(constraint) => {
                if let Some(generics) = &mut constraint.generics {
                    substitute_infer_in_angle_bracketed_arguments(generics, replacement);
                }
                substitute_infer_in_bounds(&mut constraint.bounds, replacement);
            },
            _ => {},
        }
    }
}
