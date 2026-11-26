pub mod completion;
pub mod parser;
pub mod registry;

pub use completion::{CompletionContext, CompletionEngine, CompletionItem, CompletionResult};
pub use parser::{
    AnimationAction, LayoutSelection, ParsedCommand, SaveTarget, WindowCommand, parse_command,
};
pub use registry::{CommandDescriptor, CommandRegistry, CompletionProviderKind};

#[derive(Debug, Clone, Default)]
pub struct CommandLineState {
    pub active: bool,
    pub buffer: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub draft_before_history: String,
    pub suggestions: Vec<CompletionItem>,
    pub selected_suggestion: Option<usize>,
    pub replace_range: (usize, usize),
    pub popup_open: bool,
}

impl CommandLineState {
    pub fn open(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.active = true;
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
        self.draft_before_history.clear();
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
        self.draft_before_history.clear();
        self.suggestions.clear();
        self.selected_suggestion = None;
        self.popup_open = false;
    }

    pub fn close_popup(&mut self) {
        self.popup_open = false;
        self.selected_suggestion = None;
    }

    pub fn insert_char(
        &mut self,
        c: char,
        registry: &CommandRegistry,
        ctx: &CompletionContext<'_>,
    ) {
        let pos = self.cursor.min(self.buffer.len());
        self.buffer.insert(pos, c);
        self.cursor = pos + c.len_utf8();
        self.history_index = None;
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn backspace(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.cursor == 0 || self.buffer.is_empty() {
            return;
        }
        let mut prev = self.cursor.min(self.buffer.len()) - 1;
        while prev > 0 && !self.buffer.is_char_boundary(prev) {
            prev -= 1;
        }
        self.buffer.remove(prev);
        self.cursor = prev;
        self.history_index = None;
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn delete(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.buffer.remove(self.cursor);
        self.history_index = None;
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_left(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.cursor == 0 {
            return;
        }
        let mut prev = self.cursor - 1;
        while prev > 0 && !self.buffer.is_char_boundary(prev) {
            prev -= 1;
        }
        self.cursor = prev;
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_right(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        let mut next = self.cursor + 1;
        while next < self.buffer.len() && !self.buffer.is_char_boundary(next) {
            next += 1;
        }
        self.cursor = next;
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_home(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cursor = 0;
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_end(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cursor = self.buffer.len();
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn refresh_completions(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        let res = CompletionEngine::complete(&self.buffer, self.cursor, registry, ctx);
        self.suggestions = res.items;
        self.replace_range = (res.replace_start, res.replace_end);
        self.popup_open = !self.suggestions.is_empty();
        if let Some(sel) = self.selected_suggestion
            && sel >= self.suggestions.len()
        {
            self.selected_suggestion = None;
        }
    }

    pub fn next_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        self.popup_open = true;
        self.selected_suggestion = Some(match self.selected_suggestion {
            None => 0,
            Some(i) => (i + 1) % self.suggestions.len(),
        });
    }

    pub fn prev_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        self.popup_open = true;
        self.selected_suggestion = Some(match self.selected_suggestion {
            None => self.suggestions.len() - 1,
            Some(0) => self.suggestions.len() - 1,
            Some(i) => i - 1,
        });
    }

    /// Accepts the currently highlighted completion item into `buffer`, returning `true`
    /// if the buffer changed (meaning the user accepted a suggestion rather than submitting).
    pub fn accept_selected(
        &mut self,
        registry: &CommandRegistry,
        ctx: &CompletionContext<'_>,
    ) -> bool {
        let Some(sel) = self.selected_suggestion else {
            return false;
        };
        let Some(item) = self.suggestions.get(sel).cloned() else {
            return false;
        };

        let (start, end) = self.replace_range;
        let current_slice = self.buffer.get(start..end).unwrap_or("");
        let append_space = item.expected_arg.is_some()
            && !self.buffer[end..].starts_with(' ');

        let mut replacement = item.insert_text.clone();
        if append_space {
            replacement.push(' ');
        }

        if current_slice == item.insert_text && !append_space {
            return false;
        }

        self.buffer.replace_range(start..end, &replacement);
        self.cursor = start + replacement.len();
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
        true
    }

    pub fn handle_up(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.popup_open && !self.suggestions.is_empty() && !self.buffer.is_empty() {
            match self.selected_suggestion {
                Some(0) => {
                    self.selected_suggestion = None;
                }
                Some(i) => {
                    self.selected_suggestion = Some(i - 1);
                }
                None => {
                    self.selected_suggestion = Some(self.suggestions.len() - 1);
                }
            }
            return;
        }
        self.history_prev(registry, ctx);
    }

    pub fn handle_down(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.popup_open && !self.suggestions.is_empty() && !self.buffer.is_empty() {
            match self.selected_suggestion {
                Some(i) if i + 1 >= self.suggestions.len() => {
                    self.selected_suggestion = None;
                }
                Some(i) => {
                    self.selected_suggestion = Some(i + 1);
                }
                None => {
                    self.selected_suggestion = Some(0);
                }
            }
            return;
        }
        self.history_next(registry, ctx);
    }

    pub fn history_prev(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.draft_before_history = self.buffer.clone();
        }
        let next_idx = match self.history_index {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_index = Some(next_idx);
        self.buffer = self.history[next_idx].clone();
        self.cursor = self.buffer.len();
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn history_next(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        let Some(idx) = self.history_index else {
            return;
        };
        if idx + 1 >= self.history.len() {
            self.history_index = None;
            self.buffer = self.draft_before_history.clone();
        } else {
            self.history_index = Some(idx + 1);
            self.buffer = self.history[idx + 1].clone();
        }
        self.cursor = self.buffer.len();
        self.selected_suggestion = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn submit(&mut self) -> String {
        let cmd = self.buffer.trim().to_string();
        if !cmd.is_empty() && self.history.last().map(String::as_str) != Some(&cmd) {
            self.history.push(cmd.clone());
        }
        self.cancel();
        cmd
    }
}
