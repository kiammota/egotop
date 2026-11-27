pub mod borders;
pub mod components;
pub mod container;
pub mod customizer;
pub mod fonts;
pub mod layout;
pub mod theme;
pub mod typography;
pub mod widgets;

pub use borders::{BorderChars, BorderCharsConfig, parse_border_style, validate_border_char};
pub use components::ComponentAnimationStore;
pub use container::{
    ComponentAnimationConfig, ComponentCategory, ComponentConfig, ComponentContent,
    ComponentKind, ComponentVisual, ContainerBorderConfig, ContainerConfig, ContainerDirection,
    ContainerNode, ContainerSize, default_ui_tree,
};
pub use layout::{
    ComputedLayout, PlacedComponent, PlacedContainer, compute_container_layout,
    compute_dashboard_layout, compute_inner_rect,
};
pub use theme::Theme;
pub use typography::RevealState;
