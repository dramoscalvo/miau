//! Terminal interaction model shared by input and rendering adapters.

use crate::workflow::application::decisions::{self, Drafts, Question};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    RunList,
    Streaming,
    Gate,
    Prompt(PromptKind),
    Confirm(Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Answer,
    Initial,
    Revision,
    Discussion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptEditMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptWordStyle {
    Word,
    BigWord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptOperator {
    Delete,
    Change,
    Yank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptTextObject {
    Inner,
    Around,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptPendingCommand {
    Goto,
    Operator(PromptOperator),
    TextObject {
        operator: PromptOperator,
        text_object: PromptTextObject,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptWordClass {
    Whitespace,
    Keyword,
    Punctuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PromptRegister {
    Characterwise(String),
    Linewise(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmittedPrompt {
    Initial,
    Revision(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Stop,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Flow,
    Channel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailView {
    Decisions,
    Review,
    Artifact,
    Changes,
}

/// Extract the human section without interpreting headings inside code fences.
/// Unrecognized or incomplete artifacts remain available in full as a fallback.
pub fn artifact_review(artifact: &str) -> Option<&str> {
    let artifact = artifact.trim_start();
    let (heading, body) = artifact.split_once('\n')?;
    if heading.trim_end() != "# Review" {
        return None;
    }
    let mut fence = None;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some((marker, length)) = fence {
            let count = trimmed.chars().take_while(|ch| *ch == marker).count();
            if count >= length && trimmed.chars().skip(count).all(char::is_whitespace) {
                fence = None;
            }
        } else if trimmed == "# Handoff" {
            let review = body.get(..offset)?.trim();
            return (!review.is_empty()).then_some(review);
        } else if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let length = trimmed.chars().take_while(|ch| *ch == marker).count();
            if length >= 3 {
                fence = Some((marker, length));
            }
        }
        offset += line.len();
    }
    None
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::Flow => Self::Channel,
            Self::Channel => Self::Flow,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    pub questions: Vec<Question>,
    pub decision_drafts: Drafts,
    pub decision_selected: usize,
    pub decision_error: Option<String>,
    pub mode: Mode,
    pub focus: Focus,
    pub selected: usize,
    pub viewed_node: usize,
    pub flow_scroll: u16,
    pub channel_scroll: u16,
    pub detail_view: DetailView,
    pub change_selected: usize,
    pub change_scroll: u16,
    pub prompt: String,
    pub prompt_edit_mode: PromptEditMode,
    prompt_cursor: usize,
    prompt_preferred_column: Option<usize>,
    prompt_pending_command: Option<PromptPendingCommand>,
    prompt_register: Option<PromptRegister>,
    pub pending_prompt: Option<String>,
    pub error: Option<String>,
    pub spinner: usize,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            questions: Vec::new(),
            decision_drafts: Drafts::default(),
            decision_selected: 0,
            decision_error: None,
            mode: Mode::RunList,
            focus: Focus::Flow,
            selected: 0,
            viewed_node: 0,
            flow_scroll: 0,
            channel_scroll: 0,
            detail_view: DetailView::Review,
            change_selected: 0,
            change_scroll: 0,
            prompt: String::new(),
            prompt_edit_mode: PromptEditMode::Normal,
            prompt_cursor: 0,
            prompt_preferred_column: None,
            prompt_pending_command: None,
            prompt_register: None,
            pending_prompt: None,
            error: None,
            spinner: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Tick,
    ToggleFocus,
    ToggleDetailView {
        has_review: bool,
    },
    SelectPreviousChange,
    SelectNextChange {
        last: usize,
    },
    ScrollChangeDiff(i16),
    Scroll(i16),
    SelectPrevious,
    SelectNext {
        last: usize,
    },
    ViewPreviousNode,
    ViewNextNode {
        last: usize,
    },
    ChangeMode(Mode),
    ClearPrompt,
    EnterPromptInsertMode,
    LeavePromptInsertMode,
    OpenPromptLineAbove,
    OpenPromptLineBelow,
    Input(char),
    Paste(String),
    Backspace,
    DeletePromptCharacter,
    DeletePromptToLineEnd,
    EditPromptLine {
        operator: PromptOperator,
    },
    PastePromptAfter,
    MovePromptLeft,
    MovePromptRight,
    MovePromptToStart,
    MovePromptToEnd,
    MovePromptToFirstNonBlank,
    MovePromptWordForward {
        style: PromptWordStyle,
    },
    MovePromptWordBackward {
        style: PromptWordStyle,
    },
    MovePromptWordEnd {
        style: PromptWordStyle,
    },
    EditPromptWord {
        operator: PromptOperator,
        text_object: PromptTextObject,
        style: PromptWordStyle,
    },
    MovePromptUp {
        width: usize,
    },
    MovePromptDown {
        width: usize,
    },
    Error(Option<String>),
}

pub(crate) fn prompt_rows(text: &str, width: usize) -> Vec<&str> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut start = 0;
    let mut used = 0;

    for (index, character) in text.char_indices() {
        if character == '\n' {
            rows.push(&text[start..index]);
            start = index + character.len_utf8();
            used = 0;
            continue;
        }

        let character_width = character.width().unwrap_or(0);
        if used > 0 && used + character_width > width {
            rows.push(&text[start..index]);
            start = index;
            used = 0;
        }
        used += character_width;
        if used >= width {
            let end = index + character.len_utf8();
            rows.push(&text[start..end]);
            start = end;
            used = 0;
        }
    }
    rows.push(&text[start..]);
    rows
}

pub(crate) fn prompt_cursor_position(text: &str, cursor: usize, width: usize) -> (usize, usize) {
    let width = width.max(1);
    let mut row = 0;
    let mut column = 0;

    for character in text[..cursor].chars() {
        if character == '\n' {
            row += 1;
            column = 0;
            continue;
        }
        let character_width = character.width().unwrap_or(0);
        if column > 0 && column + character_width > width {
            row += 1;
            column = 0;
        }
        column += character_width;
        if column >= width {
            row += 1;
            column = 0;
        }
    }
    (row, column)
}

impl Model {
    pub fn begin_streaming(&mut self) {
        self.mode = Mode::Streaming;
        self.detail_view = DetailView::Review;
        self.focus = Focus::Flow;
        self.flow_scroll = 0;
    }

    pub fn load_decisions(&mut self, artifact: &str) {
        self.questions.clear();
        self.decision_error = None;
        match decisions::parse(artifact) {
            Ok(questions) => self.questions = questions,
            Err(error) => self.decision_error = Some(error),
        }
        self.decision_selected = self
            .decision_selected
            .min(self.questions.len().saturating_sub(1));
        self.flow_scroll = 0;
        if !self.questions.is_empty() || self.decision_error.is_some() {
            self.detail_view = DetailView::Decisions;
        } else if self.detail_view == DetailView::Decisions {
            self.detail_view = DetailView::Review;
        }
    }

    pub fn open_answer(&mut self) {
        let Some(question) = self.questions.get(self.decision_selected) else {
            return;
        };
        let answer = self.decision_drafts.answer(question).to_owned();
        self.open_prompt(PromptKind::Answer);
        self.prompt = answer;
        self.prompt_cursor = self.prompt.len();
        self.prompt_edit_mode = PromptEditMode::Insert;
    }

    pub fn return_to_run_list(&mut self) {
        self.questions.clear();
        self.decision_drafts = Drafts::default();
        self.decision_error = None;
        self.decision_selected = 0;
        self.mode = Mode::RunList;
        self.focus = Focus::Flow;
        self.flow_scroll = 0;
        self.channel_scroll = 0;
        self.detail_view = DetailView::Review;
        self.change_selected = 0;
        self.change_scroll = 0;
        self.viewed_node = 0;
        self.prompt.clear();
        self.prompt_edit_mode = PromptEditMode::Normal;
        self.prompt_cursor = 0;
        self.prompt_preferred_column = None;
        self.prompt_pending_command = None;
        self.prompt_register = None;
        self.pending_prompt = None;
        self.error = None;
    }

    pub fn open_prompt(&mut self, kind: PromptKind) {
        self.prompt = match kind {
            PromptKind::Answer => String::new(),
            PromptKind::Initial => self.pending_prompt.clone().unwrap_or_default(),
            PromptKind::Revision => String::new(),
            PromptKind::Discussion => "Update this step's artifact to incorporate the agreed changes from our interactive discussion. Read the current artifact from disk first. Apply any agreed implementation changes if this is an implementation step. If no changes were agreed, say so; do not invent decisions. Return the complete updated artifact for human review.".into(),
        };
        self.prompt_cursor = self.prompt.len();
        self.move_prompt_left();
        self.prompt_edit_mode = PromptEditMode::Normal;
        self.prompt_preferred_column = None;
        self.prompt_pending_command = None;
        self.mode = Mode::Prompt(kind);
    }

    pub fn cancel_prompt(&mut self) {
        self.prompt.clear();
        self.prompt_edit_mode = PromptEditMode::Normal;
        self.prompt_cursor = 0;
        self.prompt_preferred_column = None;
        self.prompt_pending_command = None;
        self.mode = Mode::Gate;
    }

    pub fn submit_prompt(&mut self) -> Option<SubmittedPrompt> {
        if self.mode == Mode::Prompt(PromptKind::Answer) {
            return None;
        }
        let Mode::Prompt(kind) = self.mode else {
            return None;
        };
        let prompt = std::mem::take(&mut self.prompt);
        self.prompt_edit_mode = PromptEditMode::Normal;
        self.prompt_cursor = 0;
        self.prompt_preferred_column = None;
        self.prompt_pending_command = None;
        let submission = match kind {
            PromptKind::Answer => return None,
            PromptKind::Initial => {
                self.pending_prompt = (!prompt.is_empty()).then_some(prompt);
                SubmittedPrompt::Initial
            }
            PromptKind::Revision | PromptKind::Discussion => SubmittedPrompt::Revision(prompt),
        };
        self.mode = Mode::Gate;
        Some(submission)
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tick => self.spinner = (self.spinner + 1) % 4,
            Message::ToggleFocus => self.focus = self.focus.next(),
            Message::ToggleDetailView { has_review } => {
                self.detail_view = match self.detail_view {
                    DetailView::Decisions if has_review => DetailView::Review,
                    DetailView::Decisions => DetailView::Artifact,
                    DetailView::Review if has_review => DetailView::Artifact,
                    DetailView::Review => DetailView::Changes,
                    DetailView::Artifact => DetailView::Changes,
                    DetailView::Changes
                        if !self.questions.is_empty() || self.decision_error.is_some() =>
                    {
                        DetailView::Decisions
                    }
                    DetailView::Changes if has_review => DetailView::Review,
                    DetailView::Changes => DetailView::Artifact,
                };
                self.flow_scroll = 0;
                self.change_scroll = 0;
            }
            Message::SelectPreviousChange => {
                self.change_selected = self.change_selected.saturating_sub(1);
                self.change_scroll = 0;
            }
            Message::SelectNextChange { last } => {
                self.change_selected = (self.change_selected + 1).min(last);
                self.change_scroll = 0;
            }
            Message::ScrollChangeDiff(delta) => {
                self.change_scroll = if delta < 0 {
                    self.change_scroll.saturating_sub(delta.unsigned_abs())
                } else {
                    self.change_scroll.saturating_add(delta as u16)
                };
            }
            Message::Scroll(delta) => match self.focus {
                Focus::Flow => {
                    self.flow_scroll = if delta < 0 {
                        self.flow_scroll.saturating_sub(delta.unsigned_abs())
                    } else {
                        self.flow_scroll.saturating_add(delta as u16)
                    };
                }
                Focus::Channel => {
                    self.channel_scroll = if delta < 0 {
                        self.channel_scroll.saturating_add(delta.unsigned_abs())
                    } else {
                        self.channel_scroll.saturating_sub(delta as u16)
                    };
                }
            },
            Message::SelectPrevious => self.selected = self.selected.saturating_sub(1),
            Message::SelectNext { last } => self.selected = (self.selected + 1).min(last),
            Message::ViewPreviousNode => {
                self.viewed_node = self.viewed_node.saturating_sub(1);
                self.flow_scroll = 0;
            }
            Message::ViewNextNode { last } => {
                self.viewed_node = (self.viewed_node + 1).min(last);
                self.flow_scroll = 0;
            }
            Message::ChangeMode(mode) => self.mode = mode,
            Message::ClearPrompt => {
                self.prompt.clear();
                self.prompt_cursor = 0;
                self.prompt_preferred_column = None;
                self.prompt_pending_command = None;
            }
            Message::EnterPromptInsertMode => {
                self.prompt_edit_mode = PromptEditMode::Insert;
                self.prompt_pending_command = None;
            }
            Message::LeavePromptInsertMode => {
                self.prompt_edit_mode = PromptEditMode::Normal;
                self.prompt_pending_command = None;
                self.move_prompt_left();
            }
            Message::OpenPromptLineAbove => self.open_prompt_line_above(),
            Message::OpenPromptLineBelow => self.open_prompt_line_below(),
            Message::Input(character) if self.prompt_edit_mode == PromptEditMode::Insert => {
                self.prompt.insert(self.prompt_cursor, character);
                self.prompt_cursor += character.len_utf8();
                self.prompt_preferred_column = None;
            }
            Message::Input(_) => {}
            Message::Paste(text) if self.prompt_edit_mode == PromptEditMode::Insert => {
                self.prompt.insert_str(self.prompt_cursor, &text);
                self.prompt_cursor += text.len();
                self.prompt_preferred_column = None;
            }
            Message::Paste(_) => {}
            Message::Backspace if self.prompt_edit_mode == PromptEditMode::Insert => {
                if let Some((previous, _)) =
                    self.prompt[..self.prompt_cursor].char_indices().next_back()
                {
                    self.prompt.drain(previous..self.prompt_cursor);
                    self.prompt_cursor = previous;
                }
                self.prompt_preferred_column = None;
            }
            Message::Backspace => {}
            Message::DeletePromptCharacter => self.delete_prompt_character(),
            Message::DeletePromptToLineEnd => self.delete_prompt_to_line_end(),
            Message::EditPromptLine { operator } => self.edit_prompt_line(operator),
            Message::PastePromptAfter => self.paste_prompt_after(),
            Message::MovePromptLeft => self.move_prompt_left(),
            Message::MovePromptRight => self.move_prompt_right(),
            Message::MovePromptToStart => {
                self.prompt_cursor = 0;
                self.prompt_preferred_column = None;
            }
            Message::MovePromptToEnd => {
                self.prompt_cursor = self.prompt.len();
                self.prompt_preferred_column = None;
            }
            Message::MovePromptToFirstNonBlank => self.move_prompt_to_first_non_blank(),
            Message::MovePromptWordForward { style } => self.move_prompt_word_forward(style),
            Message::MovePromptWordBackward { style } => self.move_prompt_word_backward(style),
            Message::MovePromptWordEnd { style } => self.move_prompt_word_end(style),
            Message::EditPromptWord {
                operator,
                text_object,
                style,
            } => self.edit_prompt_word(operator, text_object, style),
            Message::MovePromptUp { width } => self.move_prompt_vertically(-1, width),
            Message::MovePromptDown { width } => self.move_prompt_vertically(1, width),
            Message::Error(error) => self.error = error,
        }
    }

    pub fn prompt_cursor(&self) -> usize {
        self.prompt_cursor
    }

    pub(crate) fn begin_prompt_g_prefix(&mut self) {
        self.prompt_pending_command = Some(PromptPendingCommand::Goto);
    }

    pub(crate) fn begin_prompt_operator(&mut self, operator: PromptOperator) {
        self.prompt_pending_command = Some(PromptPendingCommand::Operator(operator));
    }

    pub(crate) fn begin_prompt_text_object(
        &mut self,
        operator: PromptOperator,
        text_object: PromptTextObject,
    ) {
        self.prompt_pending_command = Some(PromptPendingCommand::TextObject {
            operator,
            text_object,
        });
    }

    pub(crate) fn take_prompt_pending_command(&mut self) -> Option<PromptPendingCommand> {
        self.prompt_pending_command.take()
    }

    fn move_prompt_left(&mut self) {
        if let Some((previous, _)) = self.prompt[..self.prompt_cursor].char_indices().next_back() {
            self.prompt_cursor = previous;
        }
        self.prompt_preferred_column = None;
    }

    fn move_prompt_right(&mut self) {
        if let Some(character) = self.prompt[self.prompt_cursor..].chars().next() {
            self.prompt_cursor += character.len_utf8();
        }
        self.prompt_preferred_column = None;
    }

    fn delete_prompt_character(&mut self) {
        if let Some(character) = self.prompt[self.prompt_cursor..].chars().next() {
            let end = self.prompt_cursor + character.len_utf8();
            self.prompt_register = Some(PromptRegister::Characterwise(
                self.prompt[self.prompt_cursor..end].to_owned(),
            ));
            self.prompt.drain(self.prompt_cursor..end);
        }
        self.prompt_preferred_column = None;
    }

    fn delete_prompt_to_line_end(&mut self) {
        let line_end = self.prompt[self.prompt_cursor..]
            .find('\n')
            .map_or(self.prompt.len(), |index| self.prompt_cursor + index);
        if self.prompt_cursor < line_end {
            self.prompt_register = Some(PromptRegister::Characterwise(
                self.prompt[self.prompt_cursor..line_end].to_owned(),
            ));
            self.prompt.drain(self.prompt_cursor..line_end);
        }
        self.prompt_preferred_column = None;
    }

    fn edit_prompt_line(&mut self, operator: PromptOperator) {
        let line_start = self.prompt[..self.prompt_cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = self.prompt[self.prompt_cursor..]
            .find('\n')
            .map_or(self.prompt.len(), |index| self.prompt_cursor + index);
        self.prompt_register = Some(PromptRegister::Linewise(
            self.prompt[line_start..line_end].to_owned(),
        ));

        if operator == PromptOperator::Delete {
            if line_end < self.prompt.len() {
                self.prompt.drain(line_start..=line_end);
                self.prompt_cursor = line_start.min(self.prompt.len());
            } else if line_start > 0 {
                self.prompt.drain(line_start - 1..line_end);
                self.prompt_cursor = self.prompt[..line_start - 1]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
            } else {
                self.prompt.clear();
                self.prompt_cursor = 0;
            }
        }
        self.prompt_preferred_column = None;
    }

    fn paste_prompt_after(&mut self) {
        let Some(register) = self.prompt_register.clone() else {
            return;
        };
        match register {
            PromptRegister::Characterwise(text) => {
                let insertion = self.prompt[self.prompt_cursor..]
                    .chars()
                    .next()
                    .map_or(self.prompt_cursor, |character| {
                        self.prompt_cursor + character.len_utf8()
                    });
                self.prompt.insert_str(insertion, &text);
                self.prompt_cursor = insertion
                    + text
                        .char_indices()
                        .next_back()
                        .map_or(0, |(index, _)| index);
            }
            PromptRegister::Linewise(line) => {
                let line_end = self.prompt[self.prompt_cursor..]
                    .find('\n')
                    .map_or(self.prompt.len(), |index| self.prompt_cursor + index);
                if line_end < self.prompt.len() {
                    let insertion = line_end + 1;
                    self.prompt.insert_str(insertion, &format!("{line}\n"));
                    self.prompt_cursor = insertion;
                } else if self.prompt.is_empty() {
                    self.prompt.push_str(&line);
                    self.prompt_cursor = 0;
                } else {
                    self.prompt.push('\n');
                    self.prompt_cursor = self.prompt.len();
                    self.prompt.push_str(&line);
                }
            }
        }
        self.prompt_preferred_column = None;
    }

    fn open_prompt_line_above(&mut self) {
        let line_start = self.prompt[..self.prompt_cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.prompt.insert(line_start, '\n');
        self.prompt_cursor = line_start;
        self.enter_prompt_insert_mode_after_opening_line();
    }

    fn open_prompt_line_below(&mut self) {
        let line_end = self.prompt[self.prompt_cursor..]
            .find('\n')
            .map_or(self.prompt.len(), |index| self.prompt_cursor + index);
        self.prompt.insert(line_end, '\n');
        self.prompt_cursor = line_end + 1;
        self.enter_prompt_insert_mode_after_opening_line();
    }

    fn enter_prompt_insert_mode_after_opening_line(&mut self) {
        self.prompt_edit_mode = PromptEditMode::Insert;
        self.prompt_preferred_column = None;
        self.prompt_pending_command = None;
    }

    fn move_prompt_to_first_non_blank(&mut self) {
        let line_start = self.prompt[..self.prompt_cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line = &self.prompt[line_start..];
        let line_end = line.find('\n').unwrap_or(line.len());
        self.prompt_cursor = line[..line_end]
            .char_indices()
            .find_map(|(index, character)| {
                (!character.is_whitespace()).then_some(line_start + index)
            })
            .unwrap_or(line_start);
        self.prompt_preferred_column = None;
    }

    fn move_prompt_word_forward(&mut self, style: PromptWordStyle) {
        let mut characters = self.prompt[self.prompt_cursor..].char_indices();
        let Some((_, current)) = characters.next() else {
            return;
        };
        let current_class = prompt_word_class(current, style);
        let mut crossed_whitespace = current_class == PromptWordClass::Whitespace;

        for (relative_index, character) in characters {
            let class = prompt_word_class(character, style);
            if class == PromptWordClass::Whitespace {
                crossed_whitespace = true;
            } else if crossed_whitespace || class != current_class {
                self.prompt_cursor += relative_index;
                self.prompt_preferred_column = None;
                return;
            }
        }

        self.prompt_cursor = self.prompt.len();
        self.prompt_preferred_column = None;
    }

    fn move_prompt_word_backward(&mut self, style: PromptWordStyle) {
        let mut characters = self.prompt[..self.prompt_cursor].char_indices().rev();
        let Some((mut target, character)) =
            characters.find(|(_, character)| !character.is_whitespace())
        else {
            self.prompt_cursor = 0;
            self.prompt_preferred_column = None;
            return;
        };
        let target_class = prompt_word_class(character, style);

        for (index, character) in characters {
            if prompt_word_class(character, style) != target_class {
                break;
            }
            target = index;
        }

        self.prompt_cursor = target;
        self.prompt_preferred_column = None;
    }

    fn move_prompt_word_end(&mut self, style: PromptWordStyle) {
        let Some(current) = self.prompt[self.prompt_cursor..].chars().next() else {
            return;
        };
        let after_current = self.prompt_cursor + current.len_utf8();
        let current_class = prompt_word_class(current, style);
        let continues_current_word = current_class != PromptWordClass::Whitespace
            && self.prompt[after_current..]
                .chars()
                .next()
                .is_some_and(|next| prompt_word_class(next, style) == current_class);
        let search_from = if continues_current_word {
            self.prompt_cursor
        } else {
            after_current
        };
        let Some((relative_start, first)) = self.prompt[search_from..]
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
        else {
            return;
        };
        let token_start = search_from + relative_start;
        let token_class = prompt_word_class(first, style);
        let mut target = token_start;

        for (relative_index, character) in self.prompt[token_start..].char_indices() {
            if prompt_word_class(character, style) != token_class {
                break;
            }
            target = token_start + relative_index;
        }

        self.prompt_cursor = target;
        self.prompt_preferred_column = None;
    }

    fn edit_prompt_word(
        &mut self,
        operator: PromptOperator,
        text_object: PromptTextObject,
        style: PromptWordStyle,
    ) {
        if let Some(range) =
            prompt_word_text_object_range(&self.prompt, self.prompt_cursor, text_object, style)
        {
            self.prompt_register = Some(PromptRegister::Characterwise(
                self.prompt[range.clone()].to_owned(),
            ));
            if operator != PromptOperator::Yank {
                self.prompt_cursor = range.start;
                self.prompt.drain(range);
            }
        }
        if operator == PromptOperator::Change {
            self.prompt_edit_mode = PromptEditMode::Insert;
        }
        self.prompt_preferred_column = None;
    }

    fn move_prompt_vertically(&mut self, delta: isize, width: usize) {
        let (current_row, current_column) =
            prompt_cursor_position(&self.prompt, self.prompt_cursor, width);
        let Some(target_row) = current_row.checked_add_signed(delta) else {
            return;
        };
        let preferred_column = *self.prompt_preferred_column.get_or_insert(current_column);
        let mut best = None;

        for cursor in self
            .prompt
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(self.prompt.len()))
        {
            let (row, column) = prompt_cursor_position(&self.prompt, cursor, width);
            if row == target_row {
                let distance = column.abs_diff(preferred_column);
                if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                    best = Some((cursor, distance));
                }
            }
        }
        if let Some((cursor, _)) = best {
            self.prompt_cursor = cursor;
        }
    }
}

fn prompt_word_class(character: char, style: PromptWordStyle) -> PromptWordClass {
    if character.is_whitespace() {
        PromptWordClass::Whitespace
    } else if style == PromptWordStyle::BigWord || character.is_alphanumeric() || character == '_' {
        PromptWordClass::Keyword
    } else {
        PromptWordClass::Punctuation
    }
}

fn prompt_word_text_object_range(
    text: &str,
    cursor: usize,
    text_object: PromptTextObject,
    style: PromptWordStyle,
) -> Option<std::ops::Range<usize>> {
    let probe = if cursor < text.len() {
        cursor
    } else {
        text.char_indices().next_back()?.0
    };
    let character = text[probe..].chars().next()?;
    let class = prompt_word_class(character, style);
    let mut start = probe;
    let mut end = probe + character.len_utf8();

    for (index, character) in text[..probe].char_indices().rev() {
        if prompt_word_class(character, style) != class {
            break;
        }
        start = index;
    }
    let token_tail = end;
    for (relative_index, character) in text[token_tail..].char_indices() {
        if prompt_word_class(character, style) != class {
            break;
        }
        end = token_tail + relative_index + character.len_utf8();
    }

    if text_object == PromptTextObject::Around && class != PromptWordClass::Whitespace {
        let trailing_end = text[end..]
            .char_indices()
            .take_while(|(_, character)| character.is_whitespace())
            .last()
            .map_or(end, |(index, character)| end + index + character.len_utf8());
        if trailing_end > end {
            end = trailing_end;
        } else {
            start = text[..start]
                .char_indices()
                .rev()
                .take_while(|(_, character)| character.is_whitespace())
                .last()
                .map_or(start, |(index, _)| index);
        }
    }

    Some(start..end)
}

#[cfg(test)]
mod tests {
    #[test]
    fn decisions_become_default_and_cycle_back_from_changes() {
        let mut model = super::Model::default();
        model.load_decisions("# Review\n## D1: Storage?\nStatus: Open\n# Handoff");
        assert_eq!(model.detail_view, super::DetailView::Decisions);
        model.update(super::Message::ToggleDetailView { has_review: true });
        assert_eq!(model.detail_view, super::DetailView::Review);
        model.detail_view = super::DetailView::Changes;
        model.update(super::Message::ToggleDetailView { has_review: true });
        assert_eq!(model.detail_view, super::DetailView::Decisions);
    }

    #[test]
    fn answer_editor_restores_draft_without_creating_revision() {
        let mut model = super::Model::default();
        model.load_decisions("# Review\n## D1: Storage?\nStatus: Open\n# Handoff");
        model
            .decision_drafts
            .set(&model.questions[0], "TOML".into());
        model.open_answer();
        assert_eq!(model.prompt, "TOML");
        assert_eq!(model.mode, super::Mode::Prompt(super::PromptKind::Answer));
        assert_eq!(model.submit_prompt(), None);
    }
    use super::{
        Action, DetailView, Focus, Message, Mode, Model, PromptEditMode, PromptKind,
        PromptOperator, PromptTextObject, PromptWordStyle, SubmittedPrompt,
    };

    #[test]
    fn starting_work_selects_live_output_from_every_detail_view() {
        for detail_view in [
            DetailView::Decisions,
            DetailView::Review,
            DetailView::Artifact,
            DetailView::Changes,
        ] {
            let mut model = Model {
                detail_view,
                flow_scroll: 12,
                focus: Focus::Channel,
                ..Model::default()
            };
            model.begin_streaming();
            assert_eq!(
                (
                    model.mode,
                    model.detail_view,
                    model.focus,
                    model.flow_scroll
                ),
                (Mode::Streaming, DetailView::Review, Focus::Flow, 0)
            );
        }
    }

    #[test]
    fn review_view_cycles_through_full_artifact_and_changes() {
        let mut model = Model::default();
        assert_eq!(model.detail_view, DetailView::Review);
        model.flow_scroll = 10;
        model.update(Message::ToggleDetailView { has_review: true });
        assert_eq!(
            (model.detail_view, model.flow_scroll),
            (DetailView::Artifact, 0)
        );
        model.update(Message::ToggleDetailView { has_review: true });
        assert_eq!(model.detail_view, DetailView::Changes);
        model.update(Message::ToggleDetailView { has_review: true });
        assert_eq!(model.detail_view, DetailView::Review);
    }

    #[test]
    fn detail_cycle_skips_duplicate_document_without_review() {
        let mut model = Model {
            flow_scroll: 10,
            ..Model::default()
        };
        model.update(Message::ToggleDetailView { has_review: false });
        assert_eq!(
            (model.detail_view, model.flow_scroll),
            (DetailView::Changes, 0)
        );
        model.update(Message::ToggleDetailView { has_review: false });
        assert_eq!(model.detail_view, DetailView::Artifact);
        model.update(Message::ToggleDetailView { has_review: false });
        assert_eq!(model.detail_view, DetailView::Changes);
    }

    #[test]
    fn review_extracts_only_human_section_with_unicode_and_fenced_headings() {
        let artifact = "# Review\n\nGoal: café 猫\n```text\n# Handoff\n```\nDecision: none\n\n# Handoff\nRead src/lib.rs\n";
        assert_eq!(
            super::artifact_review(artifact),
            Some("Goal: café 猫\n```text\n# Handoff\n```\nDecision: none")
        );
    }

    #[test]
    fn review_falls_back_for_legacy_or_incomplete_artifacts() {
        for artifact in [
            "Old report",
            "# Review\nPartial output",
            "# Review\n\n# Handoff\nNotes",
        ] {
            assert_eq!(super::artifact_review(artifact), None);
        }
    }

    #[test]
    fn changes_view_navigation_selects_files_and_scrolls_the_diff() {
        let mut model = Model::default();

        model.update(Message::ToggleDetailView { has_review: true });
        model.update(Message::ToggleDetailView { has_review: true });
        model.update(Message::SelectNextChange { last: 2 });
        model.update(Message::ScrollChangeDiff(10));
        model.update(Message::SelectPreviousChange);

        assert_eq!(
            (
                model.detail_view,
                model.change_selected,
                model.change_scroll
            ),
            (DetailView::Changes, 0, 0)
        );
    }

    fn model_with_prompt(prompt: &str, cursor: usize) -> Model {
        Model {
            mode: Mode::Prompt(PromptKind::Initial),
            prompt: prompt.into(),
            prompt_cursor: cursor,
            ..Model::default()
        }
    }

    #[test]
    fn opening_a_prompt_starts_in_vim_normal_mode() {
        let mut model = Model::default();

        model.open_prompt(PromptKind::Initial);

        assert_eq!(model.prompt_edit_mode, PromptEditMode::Normal);
    }

    #[test]
    fn prompt_text_changes_only_in_vim_insert_mode() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::Input('n'));
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('i'));

        assert_eq!(model.prompt, "i");
    }

    #[test]
    fn vim_delete_removes_the_character_under_the_prompt_cursor() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in "ab".chars() {
            model.update(Message::Input(character));
        }
        model.update(Message::LeavePromptInsertMode);

        model.update(Message::DeletePromptCharacter);

        assert_eq!(model.prompt, "a");
    }

    #[test]
    fn scrolling_updates_only_the_focused_pane() {
        let mut model = Model::default();
        model.update(Message::Scroll(3));
        model.update(Message::ToggleFocus);
        model.update(Message::Scroll(-2));
        assert_eq!(
            (model.flow_scroll, model.channel_scroll, model.focus),
            (3, 2, Focus::Channel)
        );
    }

    #[test]
    fn scrolling_down_moves_the_channel_back_toward_the_tail() {
        let mut model = Model {
            focus: Focus::Channel,
            channel_scroll: 3,
            ..Model::default()
        };

        model.update(Message::Scroll(1));

        assert_eq!(model.channel_scroll, 2);
    }

    #[test]
    fn upward_scrolling_saturates_at_zero() {
        let mut model = Model::default();
        model.update(Message::Scroll(-10));
        assert_eq!(model.flow_scroll, 0);
    }

    #[test]
    fn prompt_input_collects_text_and_backspace_removes_a_character() {
        let mut model = Model::default();
        model.update(Message::ChangeMode(Mode::Prompt(PromptKind::Initial)));
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('é'));
        model.update(Message::Input('🦀'));
        model.update(Message::Backspace);

        assert_eq!(
            (model.mode, model.prompt),
            (Mode::Prompt(PromptKind::Initial), "é".into())
        );
    }

    #[test]
    fn discussion_return_waits_for_human_to_submit_or_cancel() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Discussion);
        assert_eq!(model.mode, Mode::Prompt(PromptKind::Discussion));
        assert!(model.prompt.contains("agreed changes"));
        assert!(model.pending_prompt.is_none());
        let draft = model.prompt.clone();
        assert_eq!(
            model.submit_prompt(),
            Some(SubmittedPrompt::Revision(draft))
        );
        assert_eq!(model.mode, Mode::Gate);

        model.open_prompt(PromptKind::Discussion);
        model.cancel_prompt();
        assert_eq!(model.mode, Mode::Gate);
        assert!(model.pending_prompt.is_none());
        assert_eq!(model.submit_prompt(), None);
    }

    #[test]
    fn prompt_paste_inserts_multiline_unicode_text_without_submitting() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Revision);
        model.update(Message::EnterPromptInsertMode);

        model.update(Message::Paste("first line\nweb 🦀 text".into()));

        assert_eq!(
            (&model.mode, model.prompt.as_str(), model.prompt_cursor(),),
            (
                &Mode::Prompt(PromptKind::Revision),
                "first line\nweb 🦀 text",
                "first line\nweb 🦀 text".len(),
            )
        );
    }

    #[test]
    fn prompt_cursor_allows_inserting_and_deleting_inside_text() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in "ac".chars() {
            model.update(Message::Input(character));
        }
        model.update(Message::MovePromptLeft);
        model.update(Message::Input('b'));
        model.update(Message::MovePromptLeft);
        model.update(Message::Backspace);

        assert_eq!((model.prompt.as_str(), model.prompt_cursor()), ("bc", 0));
    }

    #[test]
    fn prompt_cursor_moves_vertically_across_wrapped_rows() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in "abcdef".chars() {
            model.update(Message::Input(character));
        }
        model.update(Message::MovePromptUp { width: 4 });

        assert_eq!(model.prompt_cursor(), 2);
    }

    #[test]
    fn prompt_cursor_moves_down_across_wrapped_rows() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        for character in "abcdef".chars() {
            model.update(Message::Input(character));
        }
        for _ in 0..4 {
            model.update(Message::MovePromptLeft);
        }
        model.update(Message::MovePromptDown { width: 4 });

        assert_eq!(model.prompt_cursor(), 6);
    }

    #[test]
    fn prompt_cursor_moves_right_over_a_whole_unicode_character() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('界'));
        model.update(Message::MovePromptLeft);
        model.update(Message::MovePromptRight);

        assert_eq!(model.prompt_cursor(), "界".len());
    }

    #[test]
    fn opening_a_prompt_line_below_the_last_line_appends_a_newline() {
        let mut model = model_with_prompt("héllo", "héllo".len());

        model.update(Message::OpenPromptLineBelow);

        assert_eq!(
            (
                model.prompt.as_str(),
                model.prompt_cursor(),
                model.prompt_edit_mode,
            ),
            ("héllo\n", "héllo\n".len(), PromptEditMode::Insert)
        );
    }

    #[test]
    fn opening_a_prompt_line_above_the_first_line_prepends_a_newline() {
        let mut model = model_with_prompt("héllo", 0);

        model.update(Message::OpenPromptLineAbove);

        assert_eq!(
            (
                model.prompt.as_str(),
                model.prompt_cursor(),
                model.prompt_edit_mode,
            ),
            ("\nhéllo", 0, PromptEditMode::Insert)
        );
    }

    #[test]
    fn vim_word_forward_stops_at_a_punctuation_word() {
        let mut model = model_with_prompt("héllo,  世界 next", 0);

        model.update(Message::MovePromptWordForward {
            style: PromptWordStyle::Word,
        });

        assert_eq!(model.prompt_cursor(), "héllo".len());
    }

    #[test]
    fn vim_big_word_forward_uses_only_whitespace_as_a_boundary() {
        let mut model = model_with_prompt("héllo,  世界 next", 0);

        model.update(Message::MovePromptWordForward {
            style: PromptWordStyle::BigWord,
        });

        assert_eq!(model.prompt_cursor(), "héllo,  ".len());
    }

    #[test]
    fn vim_word_backward_moves_to_the_unicode_word_start() {
        let prompt = "one,  世界 next";
        let mut model = model_with_prompt(prompt, "one,  世界 ".len());

        model.update(Message::MovePromptWordBackward {
            style: PromptWordStyle::Word,
        });

        assert_eq!(model.prompt_cursor(), "one,  ".len());
    }

    #[test]
    fn vim_word_end_stops_on_the_last_character_of_the_word() {
        let mut model = model_with_prompt("héllo, next", 0);

        model.update(Message::MovePromptWordEnd {
            style: PromptWordStyle::Word,
        });

        assert_eq!(model.prompt_cursor(), "héll".len());
    }

    #[test]
    fn vim_big_word_end_includes_punctuation() {
        let mut model = model_with_prompt("héllo, next", 0);

        model.update(Message::MovePromptWordEnd {
            style: PromptWordStyle::BigWord,
        });

        assert_eq!(model.prompt_cursor(), "héllo".len());
    }

    #[test]
    fn vim_caret_moves_to_the_first_non_blank_character() {
        let mut model = model_with_prompt(" \télan", " \télan".len());

        model.update(Message::MovePromptToFirstNonBlank);

        assert_eq!(model.prompt_cursor(), 2);
    }

    #[test]
    fn vim_delete_inner_word_removes_only_the_word_under_the_cursor() {
        let mut model = model_with_prompt("say héllo, now", "say hé".len());

        model.update(Message::EditPromptWord {
            operator: PromptOperator::Delete,
            text_object: PromptTextObject::Inner,
            style: PromptWordStyle::Word,
        });

        assert_eq!(
            (model.prompt.as_str(), model.prompt_cursor()),
            ("say , now", 4)
        );
    }

    #[test]
    fn vim_delete_around_word_prefers_trailing_whitespace() {
        let mut model = model_with_prompt("one two three", "one t".len());

        model.update(Message::EditPromptWord {
            operator: PromptOperator::Delete,
            text_object: PromptTextObject::Around,
            style: PromptWordStyle::Word,
        });

        assert_eq!(model.prompt, "one three");
    }

    #[test]
    fn vim_delete_around_last_word_removes_leading_whitespace() {
        let mut model = model_with_prompt("one two", "one two".len());

        model.update(Message::EditPromptWord {
            operator: PromptOperator::Delete,
            text_object: PromptTextObject::Around,
            style: PromptWordStyle::Word,
        });

        assert_eq!(model.prompt, "one");
    }

    #[test]
    fn vim_change_inner_word_enters_insert_mode_at_the_removed_word() {
        let mut model = model_with_prompt("one two", "one t".len());

        model.update(Message::EditPromptWord {
            operator: PromptOperator::Change,
            text_object: PromptTextObject::Inner,
            style: PromptWordStyle::Word,
        });

        assert_eq!(
            (
                model.prompt.as_str(),
                model.prompt_cursor(),
                model.prompt_edit_mode,
            ),
            ("one ", 4, PromptEditMode::Insert)
        );
    }

    #[test]
    fn stop_confirmation_is_distinct_from_quitting() {
        assert_ne!(Action::Stop, Action::Quit);
    }

    #[test]
    fn finish_confirmation_is_distinct_from_stopping() {
        assert_ne!(Action::Finish, Action::Stop);
    }

    #[test]
    fn initial_prompt_submission_is_saved_without_requesting_a_revision() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('g'));
        model.update(Message::Input('o'));

        let submission = model.submit_prompt();

        assert_eq!(
            (submission, model.mode, model.pending_prompt),
            (
                Some(SubmittedPrompt::Initial),
                Mode::Gate,
                Some("go".into())
            )
        );
    }

    #[test]
    fn reopening_initial_prompt_restores_the_saved_draft() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Initial);
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('g'));
        model.update(Message::Input('o'));
        model.submit_prompt();

        model.open_prompt(PromptKind::Initial);

        assert_eq!(model.prompt, "go");
    }

    #[test]
    fn revision_prompt_submission_remains_distinct_from_initial_prompt() {
        let mut model = Model::default();
        model.open_prompt(PromptKind::Revision);
        model.update(Message::EnterPromptInsertMode);
        model.update(Message::Input('r'));

        assert_eq!(
            model.submit_prompt(),
            Some(SubmittedPrompt::Revision("r".into()))
        );
    }

    #[test]
    fn returning_to_run_list_resets_project_view_state() {
        let mut model = Model {
            mode: Mode::Gate,
            focus: Focus::Channel,
            flow_scroll: 4,
            channel_scroll: 7,
            viewed_node: 2,
            pending_prompt: Some("draft".into()),
            error: Some("failed".into()),
            ..Model::default()
        };

        model.return_to_run_list();

        assert_eq!(
            (
                model.mode,
                model.focus,
                model.flow_scroll,
                model.channel_scroll,
                model.viewed_node,
                model.pending_prompt,
                model.error,
            ),
            (Mode::RunList, Focus::Flow, 0, 0, 0, None, None)
        );
    }

    #[test]
    fn viewing_nodes_moves_within_bounds_and_resets_artifact_scroll() {
        let mut model = Model {
            viewed_node: 1,
            flow_scroll: 8,
            ..Model::default()
        };

        model.update(Message::ViewNextNode { last: 2 });
        model.update(Message::ViewNextNode { last: 2 });
        model.update(Message::ViewPreviousNode);

        assert_eq!((model.viewed_node, model.flow_scroll), (1, 0));
    }
}
