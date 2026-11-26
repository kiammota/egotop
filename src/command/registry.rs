#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionProviderKind {
    None,
    Colorscheme,
    Animation,
    Layout,
    SetOption,
    Window,
    Effect,
    WriteTarget,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandArgSpec {
    pub name: &'static str,
    pub required: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandDescriptor {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage: &'static str,
    pub args: &'static [CommandArgSpec],
    pub provider: CompletionProviderKind,
}

#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: Vec<CommandDescriptor>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: vec![
                CommandDescriptor {
                    name: "colorscheme",
                    aliases: &["theme"],
                    description: "Apply a theme or open the theme picker",
                    usage: ":colorscheme [name]",
                    args: &[CommandArgSpec {
                        name: "<name>",
                        required: false,
                        description: "Colorscheme name from Theme Manager",
                    }],
                    provider: CompletionProviderKind::Colorscheme,
                },
                CommandDescriptor {
                    name: "set",
                    aliases: &[],
                    description: "Update runtime border or theme properties",
                    usage: ":set <property> <value>",
                    args: &[
                        CommandArgSpec {
                            name: "<property>",
                            required: true,
                            description: "Option key (e.g. border.horizontal, background)",
                        },
                        CommandArgSpec {
                            name: "<value>",
                            required: true,
                            description: "Value to apply immediately",
                        },
                    ],
                    provider: CompletionProviderKind::SetOption,
                },
                CommandDescriptor {
                    name: "animation",
                    aliases: &["anim"],
                    description: "Preview, set startup animation, or open picker",
                    usage: ":animation [play|set] [name]",
                    args: &[CommandArgSpec {
                        name: "[play|set] [name]",
                        required: false,
                        description: "Action or animation name",
                    }],
                    provider: CompletionProviderKind::Animation,
                },
                CommandDescriptor {
                    name: "layout",
                    aliases: &[],
                    description: "Switch layout mode or load a layout preset",
                    usage: ":layout [load|mode] [name]",
                    args: &[CommandArgSpec {
                        name: "[name]",
                        required: false,
                        description: "Layout mode or preset name",
                    }],
                    provider: CompletionProviderKind::Layout,
                },
                CommandDescriptor {
                    name: "window",
                    aliases: &["win"],
                    description: "Configure window anchor, borders, background, and behavior",
                    usage: ":window [subcommand] [value]",
                    args: &[CommandArgSpec {
                        name: "[subcommand]",
                        required: false,
                        description: "center, anchor, border, bg, drag, resize, padding, reset",
                    }],
                    provider: CompletionProviderKind::Window,
                },
                CommandDescriptor {
                    name: "effect",
                    aliases: &["fx"],
                    description: "Switch background ambient effect",
                    usage: ":effect [matrix|water|particles|scanlines|none]",
                    args: &[CommandArgSpec {
                        name: "[name]",
                        required: false,
                        description: "Background effect name",
                    }],
                    provider: CompletionProviderKind::Effect,
                },
                CommandDescriptor {
                    name: "reload",
                    aliases: &[],
                    description: "Reload config, themes, animations, and window state from disk",
                    usage: ":reload",
                    args: &[],
                    provider: CompletionProviderKind::None,
                },
                CommandDescriptor {
                    name: "write",
                    aliases: &["w"],
                    description: "Write runtime settings to ~/.config/egotop/",
                    usage: ":write [all|config|window|theme]",
                    args: &[CommandArgSpec {
                        name: "[target]",
                        required: false,
                        description: "Configuration target to write",
                    }],
                    provider: CompletionProviderKind::WriteTarget,
                },
                CommandDescriptor {
                    name: "help",
                    aliases: &["h"],
                    description: "Open help and keybindings reference",
                    usage: ":help",
                    args: &[],
                    provider: CompletionProviderKind::None,
                },
                CommandDescriptor {
                    name: "quit",
                    aliases: &["q"],
                    description: "Quit egotop and restore terminal state",
                    usage: ":quit",
                    args: &[],
                    provider: CompletionProviderKind::None,
                },
            ],
        }
    }

    pub fn commands(&self) -> &[CommandDescriptor] {
        &self.commands
    }

    pub fn find(&self, name_or_alias: &str) -> Option<&CommandDescriptor> {
        let needle = name_or_alias.trim().to_ascii_lowercase();
        self.commands.iter().find(|cmd| {
            cmd.name == needle || cmd.aliases.iter().any(|&a| a == needle)
        })
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
