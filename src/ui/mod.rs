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
    BackgroundFill, ComponentAnimationConfig, ComponentCategory, ComponentContent, ComponentKind,
    ContainerBorderConfig, ContainerConfig, ContainerGradient, ContainerSize, ContainerStyle,
    EffectPolicy, GradientDirection, InlineLuaScript, LayoutRegion, collect_container_names,
    default_containers, find_container, has_container, remove_container, toggle_container_visible,
};
pub use layout::{
    ComputedLayout, ExclusionMask, PlacedContainer, compute_dashboard_layout, compute_inner_rect,
    compute_layout,
};
pub use theme::Theme;
pub use typography::RevealState;
