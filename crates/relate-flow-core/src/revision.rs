use crate::model::{Step, StepState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carry {
    Keep,
    Reopen { cancel_lease: bool },
    Add,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absent {
    Refuse,
    Remove,
}

pub fn carry(previous: Option<&Step>, draft: &Step) -> Carry {
    match previous {
        None => Carry::Add,
        Some(prev) if prev.state == StepState::Succeeded && prev.done_when == draft.done_when => {
            Carry::Keep
        }
        Some(prev) => Carry::Reopen {
            cancel_lease: prev.state == StepState::Running,
        },
    }
}

pub fn absent(previous: &Step) -> Absent {
    if previous.state == StepState::Succeeded {
        Absent::Refuse
    } else {
        Absent::Remove
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, done: &str, state: StepState) -> Step {
        Step {
            uid: id.into(),
            id: id.into(),
            title: id.into(),
            done_when: done.into(),
            produces: None,
            results: vec![],
            needs: vec![],
            when: vec![],
            when_any: vec![],
            actor: None,
            state,
            position: 0,
        }
    }

    #[test]
    fn carry_table() {
        let kept = step("parse", "same", StepState::Succeeded);
        let draft = step("parse", "same", StepState::Pending);
        assert_eq!(carry(Some(&kept), &draft), Carry::Keep);

        let changed = step("parse", "different", StepState::Pending);
        assert_eq!(
            carry(Some(&kept), &changed),
            Carry::Reopen {
                cancel_lease: false
            }
        );

        let running = step("parse", "same", StepState::Running);
        assert_eq!(
            carry(Some(&running), &draft),
            Carry::Reopen { cancel_lease: true }
        );

        let blocked = step("parse", "same", StepState::Blocked);
        assert_eq!(
            carry(Some(&blocked), &draft),
            Carry::Reopen {
                cancel_lease: false
            }
        );
        assert_eq!(carry(None, &draft), Carry::Add);
    }

    #[test]
    fn absent_table() {
        assert_eq!(
            absent(&step("parse", "d", StepState::Succeeded)),
            Absent::Refuse
        );
        assert_eq!(
            absent(&step("parse", "d", StepState::Blocked)),
            Absent::Remove
        );
        assert_eq!(
            absent(&step("parse", "d", StepState::Failed)),
            Absent::Remove
        );
        assert_eq!(
            absent(&step("parse", "d", StepState::Pending)),
            Absent::Remove
        );
    }
}
