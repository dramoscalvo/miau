//! Human decision list and answer fields.

use super::pane;
use crate::{
    runs::domain::Run,
    terminal::application::{Mode, Model, PromptKind},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
};

#[cfg(test)]
mod tests;

pub fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, run: Option<&Run>) {
    frame.render_widget(Clear, area);
    let step = run
        .and_then(|run| run.nodes.get(model.viewed_node))
        .map_or("", |node| node.name.as_str());
    let title = format!(" Decisions · {step} · Enter answer · S submit answers · v review ");
    if let Some(error) = &model.decision_error {
        frame.render_widget(
            Paragraph::new(error.as_str())
                .red()
                .wrap(Wrap { trim: false })
                .block(pane::block(&title, true)),
            area,
        );
        return;
    }
    let list_height = (model.questions.len() as u16)
        .saturating_add(2)
        .min(area.height / 3)
        .max(3);
    let [list, body] =
        Layout::vertical([Constraint::Length(list_height), Constraint::Min(1)]).areas(area);
    let items: Vec<_> = model
        .questions
        .iter()
        .map(|question| {
            let answer = model.decision_drafts.answer(question);
            let status = if answer.trim().is_empty() {
                "unanswered"
            } else {
                "draft saved"
            };
            ListItem::new(format!("{}: {}  [{status}]", question.id, question.title))
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(model.decision_selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane::block(&title, true))
            .highlight_style(Style::default().bold().cyan())
            .highlight_symbol("> "),
        list,
        &mut state,
    );
    let Some(question) = model.questions.get(model.decision_selected) else {
        return;
    };
    let answer = if model.mode == Mode::Prompt(PromptKind::Answer) {
        model.prompt.as_str()
    } else {
        model.decision_drafts.answer(question)
    };
    let answer = if answer.is_empty() {
        "[Enter to write your answer]"
    } else {
        answer
    };
    let text = format!("{}\n\nYour answer:\n{}", question.question, answer);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((model.flow_scroll, 0))
            .block(pane::block(
                " Question and answer · PgUp/PgDn scroll ",
                false,
            )),
        body,
    );
}
