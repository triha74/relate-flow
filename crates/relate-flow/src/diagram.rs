//! ASCII flowchart for `show --output text`.
//!
//! Steps sit in ranks. A step's rank is one past the latest step it needs or
//! the latest step whose produce its condition names. Lines run in the gap
//! between ranks, so they do not cross a box. A line that skips a rank drops
//! down the right-hand spine.

use serde_json::Value;

const NORTH: u8 = 0b1000;
const EAST: u8 = 0b0100;
const SOUTH: u8 = 0b0010;
const WEST: u8 = 0b0001;
const BOX_INNER_ROWS: usize = 3;
const BOX_HEIGHT: usize = BOX_INNER_ROWS + 2;
const BAND: usize = 3;
const GAP: usize = 4;
const CLIP: usize = 24;

struct Step {
    id: String,
    lines: [String; BOX_INNER_ROWS],
    width: usize,
}

struct Edge {
    from: usize,
    to: usize,
    label: String,
}

struct Canvas {
    cells: Vec<Vec<char>>,
}

pub fn render(value: &Value) -> String {
    let mut out = String::new();
    if let Some(plan) = value.get("plan").and_then(Value::as_str) {
        if !plan.is_empty() {
            out.push_str(plan);
            out.push('\n');
        }
    }
    if value.get("draft") == Some(&Value::Bool(true)) {
        match value.get("basedOn").and_then(Value::as_i64) {
            Some(version) => out.push_str(&format!("draft, based on {version}\n")),
            None => out.push_str("draft\n"),
        }
    }
    if let Some(goal) = value.get("goal").and_then(Value::as_str) {
        if !goal.is_empty() {
            out.push_str(goal);
            out.push('\n');
        }
    }
    if let Some(inputs) = value.get("inputs").and_then(Value::as_object) {
        for (key, item) in inputs {
            if let Some(text) = item.as_str() {
                out.push_str(key);
                out.push_str(": ");
                out.push_str(text);
                out.push('\n');
            }
        }
    }
    out.push('\n');
    let (steps, raw) = steps_of(value);
    out.push_str(&draw(&steps, &raw));
    out
}

fn steps_of(value: &Value) -> (Vec<Step>, Vec<Value>) {
    let Some(raw) = value.get("steps").and_then(Value::as_array) else {
        return (Vec::new(), Vec::new());
    };
    let steps = raw
        .iter()
        .map(|step| {
            let id = step.get("id").and_then(Value::as_str).unwrap_or("");
            let actor = step.get("actor").and_then(Value::as_str).unwrap_or("");
            let produces = produce_line(step);
            let lines = [clip(id), clip(actor), clip(&produces)];
            let width = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(1)
                .max(1)
                + 2;
            Step {
                id: id.to_string(),
                lines,
                width,
            }
        })
        .collect();
    (steps, raw.clone())
}

fn produce_line(step: &Value) -> String {
    let Some(name) = step.get("produces").and_then(Value::as_str) else {
        return String::new();
    };
    let mut line = format!("→ {name}");
    if let Some(results) = step.get("results").and_then(Value::as_array) {
        let tokens: Vec<&str> = results.iter().filter_map(Value::as_str).collect();
        if !tokens.is_empty() {
            line.push(' ');
            line.push_str(&tokens.join("|"));
        }
    }
    line
}

fn clip(text: &str) -> String {
    let count = text.chars().count();
    if count <= CLIP {
        return text.to_string();
    }
    let mut out: String = text.chars().take(CLIP - 1).collect();
    out.push('…');
    out
}

fn draw(steps: &[Step], raw: &[Value]) -> String {
    if steps.is_empty() {
        return String::new();
    }
    let edges = edges_from(raw, steps);
    let ranks = ranks(steps.len(), &edges);
    let (x, y, layout_width) = place(steps, &ranks);
    let spine = layout_width + 1;
    let label_room = edges
        .iter()
        .map(|e| e.label.chars().count())
        .max()
        .unwrap_or(0);
    let width = spine + 2 + label_room;
    let height = y.iter().copied().max().unwrap_or(0) + BOX_HEIGHT;
    let mut canvas = Canvas::new(width, height);
    for (i, step) in steps.iter().enumerate() {
        canvas.box_at(x[i], y[i], step);
    }
    for edge in &edges {
        canvas.edge(
            x[edge.from],
            y[edge.from],
            steps[edge.from].width,
            x[edge.to],
            y[edge.to],
            steps[edge.to].width,
            spine,
            &edge.label,
        );
    }
    canvas.render()
}

fn place(steps: &[Step], ranks: &[Vec<usize>]) -> (Vec<usize>, Vec<usize>, usize) {
    let n = steps.len();
    let mut x = vec![0usize; n];
    let mut y = vec![0usize; n];
    let mut layout_width = 0usize;
    for (rank_index, rank) in ranks.iter().enumerate() {
        let mut cursor = 0usize;
        for &id in rank {
            x[id] = cursor;
            y[id] = rank_index * (BOX_HEIGHT + BAND);
            cursor += steps[id].width + 2 + GAP;
        }
        layout_width = layout_width.max(cursor.saturating_sub(GAP));
    }
    (x, y, layout_width)
}

fn ranks(n: usize, edges: &[Edge]) -> Vec<Vec<usize>> {
    let mut memo = vec![None; n];
    let mut stack = vec![false; n];
    fn depth(i: usize, edges: &[Edge], memo: &mut [Option<usize>], stack: &mut [bool]) -> usize {
        if let Some(done) = memo[i] {
            return done;
        }
        if stack[i] {
            return 0;
        }
        stack[i] = true;
        let best = edges
            .iter()
            .filter(|e| e.to == i)
            .map(|e| depth(e.from, edges, memo, stack).saturating_add(1))
            .max()
            .unwrap_or(0);
        stack[i] = false;
        memo[i] = Some(best);
        best
    }
    let mut by_rank: Vec<Vec<usize>> = Vec::new();
    for i in 0..n {
        let d = depth(i, edges, &mut memo, &mut stack);
        if by_rank.len() <= d {
            by_rank.resize(d + 1, Vec::new());
        }
        by_rank[d].push(i);
    }
    by_rank
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![' '; width.max(1)]; height.max(1)],
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for row in &self.cells {
            let line: String = row.iter().collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    fn box_at(&mut self, x: usize, y: usize, step: &Step) {
        let w = step.width;
        self.hline(x + 1, x + w, y, '─');
        self.paint(x, y, '┌');
        self.paint(x + w + 1, y, '┐');
        for (row, line) in step.lines.iter().enumerate() {
            let yy = y + 1 + row;
            self.paint(x, yy, '│');
            self.paint(x + w + 1, yy, '│');
            self.write(x + 2, yy, line);
        }
        let bottom = y + BOX_HEIGHT - 1;
        self.hline(x + 1, x + w, bottom, '─');
        self.paint(x, bottom, '└');
        self.paint(x + w + 1, bottom, '┘');
    }

    #[allow(clippy::too_many_arguments)]
    fn edge(
        &mut self,
        x1: usize,
        y1: usize,
        w1: usize,
        x2: usize,
        y2: usize,
        w2: usize,
        spine: usize,
        label: &str,
    ) {
        let c1 = x1 + (w1 + 2) / 2;
        let c2 = x2 + (w2 + 2) / 2;
        let bottom = y1 + BOX_HEIGHT - 1;
        let top = y2;
        self.paint(c1, bottom, '┴');
        if top == bottom + 1 + BAND {
            self.drop_into(c1, c2, bottom, label);
        } else if top > bottom {
            self.paint(c1, bottom + 1, '│');
            self.rail(c1, spine, bottom + 2);
            for y in (bottom + 3)..top.saturating_sub(2) {
                self.paint(spine, y, '│');
            }
            self.rail(spine, c2, top - 2);
            self.paint(c2, top - 1, '▼');
            self.write(c2 + 2, top - 1, label);
        }
    }

    fn drop_into(&mut self, from_x: usize, to_x: usize, bottom: usize, label: &str) {
        self.paint(from_x, bottom + 1, '│');
        self.rail(from_x, to_x, bottom + 2);
        self.paint(to_x, bottom + 3, '▼');
        self.write(to_x + 2, bottom + 3, label);
    }

    fn rail(&mut self, x0: usize, x1: usize, y: usize) {
        if x0 == x1 {
            self.paint(x0, y, '│');
            return;
        }
        let (left, right) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
        if right > left + 1 {
            self.hline(left + 1, right - 1, y, '─');
        }
        if x0 < x1 {
            self.paint(x0, y, '└');
            self.paint(x1, y, '┐');
        } else {
            self.paint(x0, y, '┘');
            self.paint(x1, y, '┌');
        }
    }

    fn paint(&mut self, x: usize, y: usize, ch: char) {
        if y >= self.cells.len() || x >= self.cells[y].len() {
            return;
        }
        let cur = self.cells[y][x];
        self.cells[y][x] = merge(cur, ch);
    }

    fn hline(&mut self, x0: usize, x1: usize, y: usize, ch: char) {
        let (a, b) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        for x in a..=b {
            self.paint(x, y, ch);
        }
    }

    fn write(&mut self, x: usize, y: usize, text: &str) {
        if y >= self.cells.len() {
            return;
        }
        for (i, ch) in text.chars().enumerate() {
            let xx = x + i;
            if xx >= self.cells[y].len() {
                break;
            }
            if self.cells[y][xx] == ' ' {
                self.cells[y][xx] = ch;
            } else {
                break;
            }
        }
    }
}

fn merge(old: char, new: char) -> char {
    if old == ' ' || old == new {
        return new;
    }
    let combined = bits(old) | bits(new);
    if combined == 0 {
        return old;
    }
    glyph(combined).unwrap_or(new)
}

fn bits(ch: char) -> u8 {
    match ch {
        '│' => NORTH | SOUTH,
        '─' => EAST | WEST,
        '┌' => EAST | SOUTH,
        '┐' => WEST | SOUTH,
        '└' => NORTH | EAST,
        '┘' => NORTH | WEST,
        '├' => NORTH | SOUTH | EAST,
        '┤' => NORTH | SOUTH | WEST,
        '┬' => EAST | WEST | SOUTH,
        '┴' => EAST | WEST | NORTH,
        '┼' => NORTH | EAST | SOUTH | WEST,
        _ => 0,
    }
}

fn glyph(bits: u8) -> Option<char> {
    Some(match bits {
        b if b == NORTH | SOUTH => '│',
        b if b == EAST | WEST => '─',
        b if b == EAST | SOUTH => '┌',
        b if b == WEST | SOUTH => '┐',
        b if b == NORTH | EAST => '└',
        b if b == NORTH | WEST => '┘',
        b if b == NORTH | SOUTH | EAST => '├',
        b if b == NORTH | SOUTH | WEST => '┤',
        b if b == EAST | WEST | SOUTH => '┬',
        b if b == EAST | WEST | NORTH => '┴',
        b if b == NORTH | EAST | SOUTH | WEST => '┼',
        _ => return None,
    })
}

fn edges_from(value_steps: &[Value], ids: &[Step]) -> Vec<Edge> {
    let index: std::collections::HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let producer: std::collections::HashMap<&str, usize> = value_steps
        .iter()
        .enumerate()
        .filter_map(|(i, step)| {
            let name = step.get("produces").and_then(Value::as_str)?;
            Some((name, i))
        })
        .collect();
    let mut grouped: std::collections::BTreeMap<(usize, usize), Vec<String>> =
        std::collections::BTreeMap::new();
    let mut add = |from: usize, to: usize, label: String| {
        if from != to {
            grouped.entry((from, to)).or_default().push(label);
        }
    };
    for (to, step) in value_steps.iter().enumerate() {
        if let Some(needs) = step.get("needs").and_then(Value::as_array) {
            for need in needs.iter().filter_map(Value::as_str) {
                if let Some(&from) = index.get(need) {
                    add(from, to, String::new());
                }
            }
        }
        if let Some(when) = step.get("when").and_then(Value::as_array) {
            for pair in when.iter().filter_map(Value::as_str) {
                add_condition(&mut add, &producer, to, pair);
            }
        }
        if let Some(any) = step.get("whenAny").and_then(Value::as_array) {
            for alt in any.iter().filter_map(Value::as_str) {
                add_condition(&mut add, &producer, to, alt);
            }
        }
    }
    grouped
        .into_iter()
        .map(|((from, to), labels)| {
            let text: Vec<&str> = labels
                .iter()
                .filter(|l| !l.is_empty())
                .map(String::as_str)
                .collect();
            let label = if text.is_empty() {
                "needs".to_string()
            } else {
                text.join(" | ")
            };
            Edge { from, to, label }
        })
        .collect()
}

fn add_condition(
    add: &mut impl FnMut(usize, usize, String),
    producer: &std::collections::HashMap<&str, usize>,
    to: usize,
    spec: &str,
) {
    for pair in spec.split(',') {
        let Some((name, token)) = pair.split_once('=') else {
            continue;
        };
        if let Some(&from) = producer.get(name) {
            add(from, to, format!("{name}={token}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_fork_draws_both_branches() {
        let text = render(&json!({
            "draft": true,
            "goal": "Restore the service, or escalate it",
            "retries": 2,
            "steps": [
                {"id": "triage", "actor": "oncall", "produces": "severity", "results": ["sev1", "sev2", "sev3"]},
                {"id": "page-ic", "actor": "ic", "needs": ["triage"], "when": ["severity=sev1"], "produces": "page"},
                {"id": "mitigate", "actor": "oncall", "needs": ["triage"], "produces": "mitigation", "results": ["resolved", "unresolved"]},
                {"id": "close-ticket", "actor": "oncall", "whenAny": ["mitigation=resolved"]}
            ]
        }));
        assert!(text.starts_with("draft\n"), "{text}");
        assert!(text.contains("triage"));
        assert!(text.contains("page-ic"));
        assert!(text.contains("mitigate"));
        assert!(text.contains("close-ticket"));
        assert!(text.contains("severity=sev1"));
        assert!(text.contains("mitigation=resolved"));
        assert!(text.contains('┌') && text.contains('┘'));
        assert!(text.contains('▼'));
    }

    #[test]
    fn a_skipped_rank_uses_the_spine_and_a_revision_names_its_base() {
        let text = render(&json!({
            "draft": true,
            "basedOn": 2,
            "goal": "Ship it",
            "steps": [
                {"id": "a", "produces": "out", "results": ["yes"]},
                {"id": "b", "needs": ["a"], "produces": "mid"},
                {"id": "c", "needs": ["b"], "when": ["out=yes"]}
            ]
        }));
        assert!(text.starts_with("draft, based on 2\n"));
        assert!(text.contains("out=yes"));
        assert!(text.contains("needs"));
    }

    #[test]
    fn an_empty_plan_is_the_goal_only() {
        let text = render(&json!({"goal": "Nothing yet", "steps": []}));
        assert_eq!(text, "Nothing yet\n\n");
    }
}
