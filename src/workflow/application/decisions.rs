//! Decision extraction and persisted human answer drafts.

use crate::runs::application::{ArtifactRepository, RepositoryError};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub title: String,
    pub question: String,
}

/// Parse the shared Markdown contract. Fences and non-Review sections are inert.
pub fn parse(artifact: &str) -> Result<Vec<Question>, String> {
    let mut questions = Vec::new();
    let mut seen = HashSet::new();
    let mut review = false;
    let mut fence = None;
    let mut current: Option<Question> = None;
    let mut open = None;
    let finish = |current: &mut Option<Question>,
                  open: &mut Option<bool>,
                  questions: &mut Vec<Question>|
     -> Result<(), String> {
        if let Some(mut question) = current.take() {
            question.question = question.question.trim().to_owned();
            let is_open = open.take().ok_or_else(|| {
                format!(
                    "{} needs Status: Open or Status: Resolved. Edit the artifact to correct it.",
                    question.id
                )
            })?;
            if is_open {
                questions.push(question);
            }
        }
        Ok(())
    };
    for line in artifact.lines() {
        let trimmed = line.trim();
        if let Some(question) = current.as_mut() {
            // Append below after handling section boundaries.
            if fence.is_some() {
                question.question.push_str(line);
                question.question.push('\n');
            }
        }
        if let Some((marker, length)) = fence {
            let count = trimmed.chars().take_while(|ch| *ch == marker).count();
            if count >= length && trimmed.chars().skip(count).all(char::is_whitespace) {
                fence = None;
            }
            continue;
        }
        if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let length = trimmed.chars().take_while(|ch| *ch == marker).count();
            if length >= 3 {
                fence = Some((marker, length));
            }
        }
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            finish(&mut current, &mut open, &mut questions)?;
            if trimmed.starts_with("# ") {
                review = trimmed == "# Review";
            } else if review
                && let Some((id, title)) =
                    trimmed.strip_prefix("## ").and_then(|s| s.split_once(":"))
                && let Some(number) = id.strip_prefix('D')
                && !number.is_empty()
                && number.bytes().all(|b| b.is_ascii_digit())
                && !number.starts_with('0')
                && !title.trim().is_empty()
            {
                if !seen.insert(id.to_owned()) {
                    return Err(format!(
                        "Duplicate decision {id}; edit the artifact to give each decision a unique ID."
                    ));
                }
                current = Some(Question {
                    id: id.to_owned(),
                    title: title.trim().to_owned(),
                    question: String::new(),
                });
            }
        }
        if let Some(question) = current.as_mut() {
            if trimmed.starts_with("Status:") {
                let status = match trimmed {
                    "Status: Open" => true,
                    "Status: Resolved" => false,
                    _ => {
                        return Err(format!(
                            "{} needs Status: Open or Status: Resolved.",
                            question.id
                        ));
                    }
                };
                if open.replace(status).is_some() {
                    return Err(format!(
                        "{} has multiple Status lines. Keep exactly one.",
                        question.id
                    ));
                }
            }
            question.question.push_str(line);
            question.question.push('\n');
        }
    }
    finish(&mut current, &mut open, &mut questions)?;
    Ok(questions)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Drafts {
    entries: Vec<Answer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Answer {
    question: String,
    text: String,
}

impl Drafts {
    pub fn answer(&self, question: &Question) -> &str {
        self.entries
            .iter()
            .find(|entry| entry.question == question.question)
            .map_or("", |entry| entry.text.as_str())
    }

    pub fn set(&mut self, question: &Question, text: String) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.question == question.question)
        {
            entry.text = text;
        } else {
            self.entries.push(Answer {
                question: question.question.clone(),
                text,
            });
        }
    }

    pub fn feedback(&self, questions: &[Question]) -> Option<String> {
        let answers: Vec<_> = questions
            .iter()
            .filter_map(|question| {
                let answer = self.answer(question);
                (!answer.trim().is_empty())
                    .then(|| format!("{}\n\nHuman answer:\n{}", question.question, answer))
            })
            .collect();
        (!answers.is_empty()).then(|| format!(
            "# Decision answers\n\nApply these human answers and return the complete updated artifact for review. Mark answered decisions Resolved and record their answers. Keep unanswered decisions Open with stable IDs. This submission does not approve the workflow step.\n\n{}\n", answers.join("\n\n")
        ))
    }

    pub fn load(
        repo: &impl ArtifactRepository,
        run: &str,
        step: usize,
    ) -> Result<Self, RepositoryError> {
        let name = format!("decision-drafts-{step}.json");
        let text = repo.read(run, &name)?;
        if text.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(&text).map_err(|source| RepositoryError::Json {
            path: std::path::PathBuf::from(name),
            source,
        })
    }

    pub fn save(
        &self,
        repo: &impl ArtifactRepository,
        run: &str,
        step: usize,
    ) -> Result<(), RepositoryError> {
        let name = format!("decision-drafts-{step}.json");
        let text = serde_json::to_string_pretty(self).map_err(|source| RepositoryError::Json {
            path: std::path::PathBuf::from(&name),
            source,
        })?;
        repo.write(run, &name, &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = "# Review\n## D1: Storage?\n\nStatus: Open\nContext: Settings.\nRecommendation: TOML.\n\n## D2: Migration?\nStatus: Resolved\nAnswer: No.\n# Handoff\n## D3: Internal?\nStatus: Open\n";

    #[test]
    fn extracts_only_open_review_decisions() {
        let decisions = parse(DOCUMENT).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].id, "D1");
        assert!(decisions[0].question.contains("Recommendation: TOML."));
    }

    #[test]
    fn ignores_fenced_examples_and_non_decision_sections() {
        let text = "# Review\n````md\n## D9: Example\nStatus: Open\n```\n````\n## D1: Real?\nStatus: Open\n### Options\nOne.\n## Verification\npassed\n# Handoff";
        let decisions = parse(text).unwrap();
        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].question.contains("One."));
        assert!(!decisions[0].question.contains("passed"));
    }

    #[test]
    fn duplicate_ids_are_reported() {
        assert!(
            parse("# Review\n## D1: One?\nStatus: Open\n## D1: Two?\nStatus: Open\n# Handoff")
                .is_err()
        );
    }

    #[test]
    fn status_inside_code_does_not_reopen_a_resolved_decision() {
        assert!(
            parse("# Review\n## D1: Example?\nStatus: Resolved\n```\nStatus: Open\n```\n# Handoff")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn missing_status_is_reported_instead_of_hiding_a_question() {
        assert!(parse("# Review\n## D1: Choose?\nRecommendation: Yes.\n# Handoff").is_err());
    }

    #[test]
    fn shared_contract_requires_stable_decisions_for_every_role() {
        let contract = include_str!("../artifact-contract.md");
        for required in [
            "## D1:",
            "Status: Open",
            "Status: Resolved",
            "stable",
            "unanswered",
        ] {
            assert!(contract.contains(required), "missing {required}");
        }
    }

    #[test]
    fn partial_feedback_includes_question_and_multiline_answer() {
        let decisions = parse(DOCUMENT).unwrap();
        let mut drafts = Drafts::default();
        drafts.set(&decisions[0], "TOML\nKeep existing settings.".into());
        let feedback = drafts.feedback(&decisions).unwrap();
        assert!(feedback.contains("## D1: Storage?"));
        assert!(feedback.contains("TOML\nKeep existing settings."));
        assert!(!feedback.contains("D2"));
    }

    #[test]
    fn changed_question_does_not_reuse_old_answer() {
        let decisions = parse(DOCUMENT).unwrap();
        let mut drafts = Drafts::default();
        drafts.set(&decisions[0], "Yes".into());
        let changed = parse(&DOCUMENT.replace("Storage?", "Delete settings?")).unwrap();
        assert!(drafts.feedback(&changed).is_none());
        assert_eq!(drafts.answer(&decisions[0]), "Yes");
    }

    #[test]
    fn drafts_survive_reopening_and_are_scoped_to_step() {
        use crate::runs::infrastructure::FileRepository;
        let dir = std::env::temp_dir().join(format!(
            "miau-decision-drafts-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let repo = FileRepository::new(&dir);
        let decisions = parse(DOCUMENT).unwrap();
        let mut drafts = Drafts::default();
        drafts.set(&decisions[0], "Sí\nTOML".into());
        drafts.save(&repo, "001", 0).unwrap();
        assert_eq!(
            Drafts::load(&repo, "001", 0).unwrap().answer(&decisions[0]),
            "Sí\nTOML"
        );
        assert!(
            Drafts::load(&repo, "001", 1)
                .unwrap()
                .feedback(&decisions)
                .is_none()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
