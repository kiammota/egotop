pub mod completion;
pub mod parser;
pub mod registry;

pub use completion::{CompletionContext, CompletionEngine, CompletionItem, CompletionResult};
pub use parser::{
    AnimationAction, BackgroundCommand, BorderCommand, ContainerCommand, ContainerProperty,
    LayoutSelection, ParsedCommand, WriteTarget, parse_command,
};
pub use registry::{CommandDescriptor, CommandRegistry, CompletionProviderKind};

#[derive(Debug, Clone)]
pub struct CompletionCycle {
    pub original_buffer: String,
    pub original_cursor: usize,
    pub token_start: usize,
    pub token_end: usize,
    pub items: Vec<CompletionItem>,
    pub selected: usize,
}

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
    pub cycle: Option<CompletionCycle>,
}

impl CommandLineState {
    pub fn open(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.active = true;
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
        self.draft_before_history.clear();
        self.selected_suggestion = None;
        self.cycle = None;
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
        self.cycle = None;
    }

    pub fn close_popup(&mut self) {
        self.popup_open = false;
        self.selected_suggestion = None;
        self.cycle = None;
    }

    pub fn insert_char(
        &mut self,
        c: char,
        registry: &CommandRegistry,
        ctx: &CompletionContext<'_>,
    ) {
        self.cycle = None;
        self.selected_suggestion = None;
        let pos = self.cursor.min(self.buffer.len());
        self.buffer.insert(pos, c);
        self.cursor = pos + c.len_utf8();
        self.history_index = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn backspace(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cycle = None;
        self.selected_suggestion = None;
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
        self.refresh_completions(registry, ctx);
    }

    pub fn delete(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cycle = None;
        self.selected_suggestion = None;
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.buffer.remove(self.cursor);
        self.history_index = None;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_left(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cycle = None;
        self.selected_suggestion = None;
        if self.cursor == 0 {
            return;
        }
        let mut prev = self.cursor - 1;
        while prev > 0 && !self.buffer.is_char_boundary(prev) {
            prev -= 1;
        }
        self.cursor = prev;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_right(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cycle = None;
        self.selected_suggestion = None;
        if self.cursor >= self.buffer.len() {
            return;
        }
        let mut next = self.cursor + 1;
        while next < self.buffer.len() && !self.buffer.is_char_boundary(next) {
            next += 1;
        }
        self.cursor = next;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_home(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cycle = None;
        self.selected_suggestion = None;
        self.cursor = 0;
        self.refresh_completions(registry, ctx);
    }

    pub fn move_end(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.cycle = None;
        self.selected_suggestion = None;
        self.cursor = self.buffer.len();
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

    /// Shell-style Tab completion: immediately completes the current token and
    /// advances through the preserved suggestion list without requiring Enter.
    pub fn next_suggestion(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.step_cycle(true, registry, ctx);
    }

    /// Shell-style Shift+Tab completion: steps backward through suggestions
    /// and immediately updates the active token in the command buffer.
    pub fn prev_suggestion(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        self.step_cycle(false, registry, ctx);
    }

    fn step_cycle(
        &mut self,
        forward: bool,
        registry: &CommandRegistry,
        ctx: &CompletionContext<'_>,
    ) {
        if self.cycle.is_none() {
            if self.suggestions.is_empty() {
                self.refresh_completions(registry, ctx);
            }
            if self.suggestions.is_empty() {
                return;
            }
            let (start, end) = self.replace_range;
            let items = self.suggestions.clone();
            let initial_sel = if forward { 0 } else { items.len() - 1 };
            self.cycle = Some(CompletionCycle {
                original_buffer: self.buffer.clone(),
                original_cursor: self.cursor,
                token_start: start,
                token_end: end,
                items,
                selected: initial_sel,
            });
        } else if let Some(cycle) = &mut self.cycle {
            let len = cycle.items.len();
            if len == 0 {
                return;
            }
            cycle.selected = if forward {
                (cycle.selected + 1) % len
            } else if cycle.selected == 0 {
                len - 1
            } else {
                cycle.selected - 1
            };
        }

        if let Some(cycle) = &mut self.cycle {
            let item = &cycle.items[cycle.selected];
            let start = cycle.token_start.min(self.buffer.len());
            let end = cycle.token_end.min(self.buffer.len());
            self.buffer.replace_range(start..end, &item.insert_text);
            let new_end = start + item.insert_text.len();
            cycle.token_end = new_end;
            self.cursor = new_end;
            self.replace_range = (start, new_end);
            self.suggestions = cycle.items.clone();
            self.selected_suggestion = Some(cycle.selected);
            self.popup_open = true;
            self.history_index = None;
        }
    }

    pub fn handle_up(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.popup_open && !self.suggestions.is_empty() && !self.buffer.is_empty() {
            self.prev_suggestion(registry, ctx);
            return;
        }
        self.history_prev(registry, ctx);
    }

    pub fn handle_down(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.popup_open && !self.suggestions.is_empty() && !self.buffer.is_empty() {
            self.next_suggestion(registry, ctx);
            return;
        }
        self.history_next(registry, ctx);
    }

    pub fn history_prev(&mut self, registry: &CommandRegistry, ctx: &CompletionContext<'_>) {
        if self.history.is_empty() {
            return;
        }
        self.cycle = None;
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
        self.cycle = None;
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
