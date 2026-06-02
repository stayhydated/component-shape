//! Framework-neutral component shape metadata and naming utilities.

mod component_suffix;
mod field_component;
mod metadata;
mod rust_syntax;
mod value_change;

pub use component_suffix::{
    ComponentSuffix, ComponentSuffixError, is_valid_component_suffix, validate_component_suffix,
};
pub use field_component::ComponentShapeUse;
pub use metadata::{
    ComponentCapabilities, ComponentPrototyping, RenderCapability, ValueBindingCapability,
};
pub use rust_syntax::{RustExpr, RustPath, RustSyntaxError, RustSyntaxKind, RustType};
pub use value_change::ValueChange;

/// Framework-neutral metadata published by a component shape.
pub trait ComponentShapeMetadata {
    const PROTOTYPING: ComponentPrototyping = ComponentPrototyping::new();
    const CAPABILITIES: ComponentCapabilities = ComponentCapabilities::new();
}
