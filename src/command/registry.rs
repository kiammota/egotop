#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionProviderKind {
    None,
    Colorscheme,
    Animation,
    Background,
    Layout,
    Container,
    SetOption,
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
                    description: "Play, pause, resume, stop, speed-scale, or pick animations",
                    usage: ":animation [play|set|pause|resume|stop|restart|speed|list] [arg]",
                    args: &[CommandArgSpec {
                        name: "[action] [arg]",
                        required: false,
                        description: "Action (play, pause, resume, stop, speed, set) or name",
                    }],
                    provider: CompletionProviderKind::Animation,
                },
                CommandDescriptor {
                    name: "background",
                    aliases: &["bg"],
                    description: "Configure, layer, pause, resume, or stop animated backgrounds",
                    usage: ":background <set|add|stop|pause|resume|speed|list> [arg]",
                    args: &[CommandArgSpec {
                        name: "[action] [arg]",
                        required: false,
                        description: "set, add, stop, pause, resume, speed, or preset name",
                    }],
                    provider: CompletionProviderKind::Background,
                },
                CommandDescriptor {
                    name: "layout",
                    aliases: &[],
                    description: "Switch view mode or load a container layout preset",
                    usage: ":layout [load|mode] [name]",
                    args: &[CommandArgSpec {
                        name: "[name]",
                        required: false,
                        description: "View mode or layout preset name",
                    }],
                    provider: CompletionProviderKind::Layout,
                },
                CommandDescriptor {
                    name: "container",
                    aliases: &[],
                    description: "Inspect and customize individual container styles",
                    usage: ":container <list|inspect|set|gradient|reset> ...",
                    args: &[CommandArgSpec {
                        name: "<subcommand>",
                        required: false,
                        description: "list, inspect, set, gradient, reset",
                    }],
                    provider: CompletionProviderKind::Container,
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
                    aliases: &["r"],
                    description: "Manually reload and validate all watched configuration files",
                    usage: ":reload",
                    args: &[],
                    provider: CompletionProviderKind::None,
                },
                CommandDescriptor {
                    name: "messages",
                    aliases: &["msg"],
                    description: "View configuration history, logs, and diagnostic messages",
                    usage: ":messages",
                    args: &[],
                    provider: CompletionProviderKind::None,
                },
                CommandDescriptor {
                    name: "errors",
                    aliases: &["err"],
                    description: "Inspect active configuration validation and load errors",
                    usage: ":errors",
                    args: &[],
                    provider: CompletionProviderKind::None,
                },
                CommandDescriptor {
                    name: "write",
                    aliases: &["w"],
                    description: "Write runtime settings to ~/.config/egotop/",
                    usage: ":write [all|config|theme]",
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
