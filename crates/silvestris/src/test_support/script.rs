//! Text-script recording and replay for interaction scenarios.
//!
//! A script is a line-oriented sequence of driver actions and assertions. Bind
//! names to focus handles and selectors in a `ScriptEnv`, then replay the script
//! against a `Scenario`. A `Recorder` writes the same format while a test runs,
//! so a failing sequence can be pasted into a bug report.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use gpui::FocusHandle;

use super::scenario::Scenario;

/// A named focus handle or debug selector for a script.
#[derive(Default)]
pub struct ScriptEnv {
    handles: BTreeMap<String, FocusHandle>,
    selectors: BTreeMap<String, &'static str>,
}

impl ScriptEnv {
    /// Creates an empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a script name to a focus handle.
    pub fn bind_focus(&mut self, name: impl Into<String>, handle: FocusHandle) -> &mut Self {
        self.handles.insert(name.into(), handle);
        self
    }

    /// Binds a script name to a stable debug selector.
    pub fn bind_selector(&mut self, name: impl Into<String>, selector: &'static str) -> &mut Self {
        self.selectors.insert(name.into(), selector);
        self
    }

    fn handle(&self, name: &str) -> Option<FocusHandle> {
        self.handles.get(name).cloned()
    }

    fn selector(&self, name: &str) -> Option<&'static str> {
        self.selectors.get(name).copied()
    }
}

/// A parse or replay failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    line: Option<usize>,
    message: String,
}

impl ScriptError {
    fn at_line(line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            message: message.into(),
        }
    }

    fn at_step(step: usize, message: impl Into<String>) -> Self {
        Self {
            line: None,
            message: format!("step {step}: {}", message.into()),
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ScriptError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Activate(usize),
    Focus(String),
    Type(String),
    Press(String),
    Click(String),
    Advance(u64),
    ExpectFocused(String),
    ExpectNoFocus,
    ExpectSelector(String),
    ExpectWindowCount(usize),
    ExpectTitle(String),
}

impl Step {
    fn text(&self) -> String {
        match self {
            Self::Activate(index) => format!("activate {index}"),
            Self::Focus(name) => format!("focus {name}"),
            Self::Type(text) => format!("type {}", escape(text)),
            Self::Press(keys) => format!("press {keys}"),
            Self::Click(name) => format!("click {name}"),
            Self::Advance(millis) => format!("advance {millis}"),
            Self::ExpectFocused(name) => format!("expect-focus {name}"),
            Self::ExpectNoFocus => "expect-no-focus".into(),
            Self::ExpectSelector(name) => format!("expect-selector {name}"),
            Self::ExpectWindowCount(count) => format!("expect-window-count {count}"),
            Self::ExpectTitle(title) => format!("expect-title {}", escape(title)),
        }
    }
}

/// A parsed or recorded interaction script.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScenarioScript {
    steps: Vec<Step>,
}

impl ScenarioScript {
    /// Parses a line-oriented script.
    ///
    /// Empty lines and lines starting with `#` are ignored. `type` and
    /// `expect-title` unescape `\\n`, `\\t` and `\\\\`.
    pub fn parse(text: &str) -> Result<Self, ScriptError> {
        let mut steps = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line_number = index + 1;
            let (command, argument) = split(line);
            let step = match command {
                "activate" => Step::Activate(parse_number(line_number, "activate", argument)?),
                "focus" => Step::Focus(required(line_number, "focus", argument)?),
                "type" => Step::Type(unescape(argument)),
                "press" => Step::Press(required(line_number, "press", argument)?),
                "click" => Step::Click(required(line_number, "click", argument)?),
                "advance" => Step::Advance(parse_number(line_number, "advance", argument)?),
                "expect-focus" => {
                    Step::ExpectFocused(required(line_number, "expect-focus", argument)?)
                }
                "expect-no-focus" => {
                    if !argument.is_empty() {
                        return Err(ScriptError::at_line(
                            line_number,
                            "expect-no-focus takes no argument",
                        ));
                    }
                    Step::ExpectNoFocus
                }
                "expect-selector" => {
                    Step::ExpectSelector(required(line_number, "expect-selector", argument)?)
                }
                "expect-window-count" => Step::ExpectWindowCount(parse_number(
                    line_number,
                    "expect-window-count",
                    argument,
                )?),
                "expect-title" => Step::ExpectTitle(unescape(argument)),
                other => {
                    return Err(ScriptError::at_line(
                        line_number,
                        format!("unknown command {other:?}"),
                    ));
                }
            };
            steps.push(step);
        }
        Ok(Self { steps })
    }

    /// Replays the script against a scenario and environment.
    pub fn replay(&self, scenario: &mut Scenario, env: &ScriptEnv) -> Result<(), ScriptError> {
        for (index, step) in self.steps.iter().enumerate() {
            let step_number = index + 1;
            match step {
                Step::Activate(window_index) => {
                    let window =
                        scenario
                            .windows()
                            .get(*window_index)
                            .copied()
                            .ok_or_else(|| {
                                ScriptError::at_step(
                                    step_number,
                                    format!("no window at index {window_index}"),
                                )
                            })?;
                    scenario.activate(window);
                }
                Step::Focus(name) => {
                    let handle = env.handle(name).ok_or_else(|| {
                        ScriptError::at_step(step_number, format!("no focus handle {name:?}"))
                    })?;
                    scenario.focus(&handle);
                }
                Step::Type(text) => scenario.type_text(text),
                Step::Press(keys) => scenario.press(keys),
                Step::Click(name) => {
                    let selector = env.selector(name).ok_or_else(|| {
                        ScriptError::at_step(step_number, format!("no selector {name:?}"))
                    })?;
                    scenario.click(selector);
                }
                Step::Advance(millis) => scenario.advance(Duration::from_millis(*millis)),
                Step::ExpectFocused(name) => {
                    let handle = env.handle(name).ok_or_else(|| {
                        ScriptError::at_step(step_number, format!("no focus handle {name:?}"))
                    })?;
                    let focused = scenario.update(|window, _| handle.is_focused(window));
                    if !focused {
                        return Err(ScriptError::at_step(
                            step_number,
                            format!("expected {name:?} to be focused"),
                        ));
                    }
                }
                Step::ExpectNoFocus => {
                    if scenario.focused().is_some() {
                        return Err(ScriptError::at_step(
                            step_number,
                            "expected no focused element",
                        ));
                    }
                }
                Step::ExpectSelector(name) => {
                    let selector = env.selector(name).ok_or_else(|| {
                        ScriptError::at_step(step_number, format!("no selector {name:?}"))
                    })?;
                    if scenario.try_bounds(selector).is_none() {
                        return Err(ScriptError::at_step(
                            step_number,
                            format!("expected selector {name:?} to render"),
                        ));
                    }
                }
                Step::ExpectWindowCount(expected) => {
                    let actual = scenario.windows().len();
                    if actual != *expected {
                        return Err(ScriptError::at_step(
                            step_number,
                            format!("expected {expected} windows, found {actual}"),
                        ));
                    }
                }
                Step::ExpectTitle(expected) => {
                    let actual = scenario.window_title();
                    if actual.as_deref() != Some(expected.as_str()) {
                        return Err(ScriptError::at_step(
                            step_number,
                            format!("expected title {expected:?}, found {actual:?}"),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl fmt::Display for ScenarioScript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for step in &self.steps {
            writeln!(f, "{}", step.text())?;
        }
        Ok(())
    }
}

/// Records driver actions and assertions as a `ScenarioScript`.
pub struct Recorder<'a> {
    scenario: &'a mut Scenario,
    steps: Vec<Step>,
}

impl<'a> Recorder<'a> {
    /// Starts recording against a scenario.
    pub fn new(scenario: &'a mut Scenario) -> Self {
        Self {
            scenario,
            steps: Vec::new(),
        }
    }

    /// Activates a window by open order.
    pub fn activate(&mut self, index: usize) {
        let window = self
            .scenario
            .windows()
            .get(index)
            .copied()
            .unwrap_or_else(|| panic!("no window at index {index}"));
        self.scenario.activate(window);
        self.steps.push(Step::Activate(index));
    }

    /// Focuses a handle and records its script name.
    pub fn focus(&mut self, name: &str, handle: &FocusHandle) {
        self.scenario.focus(handle);
        self.steps.push(Step::Focus(name.into()));
    }

    /// Types text.
    pub fn type_text(&mut self, text: &str) {
        self.scenario.type_text(text);
        self.steps.push(Step::Type(text.into()));
    }

    /// Presses a key chord.
    pub fn press(&mut self, keys: &str) {
        self.scenario.press(keys);
        self.steps.push(Step::Press(keys.into()));
    }

    /// Clicks a selector and records its script name.
    pub fn click(&mut self, name: &str, selector: &'static str) {
        self.scenario.click(selector);
        self.steps.push(Step::Click(name.into()));
    }

    /// Advances the test clock.
    pub fn advance(&mut self, duration: Duration) {
        self.scenario.advance(duration);
        self.steps.push(Step::Advance(duration.as_millis() as u64));
    }

    /// Asserts focus and records its script name.
    pub fn expect_focus(&mut self, name: &str, handle: &FocusHandle) {
        self.scenario.assert_focused(handle);
        self.steps.push(Step::ExpectFocused(name.into()));
    }

    /// Asserts that nothing is focused.
    pub fn expect_no_focus(&mut self) {
        self.scenario.assert_no_focus();
        self.steps.push(Step::ExpectNoFocus);
    }

    /// Asserts that a selector renders and records its script name.
    pub fn expect_selector(&mut self, name: &str, selector: &'static str) {
        assert!(
            self.scenario.try_bounds(selector).is_some(),
            "no element for selector {selector:?}"
        );
        self.steps.push(Step::ExpectSelector(name.into()));
    }

    /// Asserts the open-window count.
    pub fn expect_window_count(&mut self, count: usize) {
        assert_eq!(self.scenario.windows().len(), count);
        self.steps.push(Step::ExpectWindowCount(count));
    }

    /// Asserts the active window's title.
    pub fn expect_title(&mut self, title: &str) {
        assert_eq!(self.scenario.window_title().as_deref(), Some(title));
        self.steps.push(Step::ExpectTitle(title.into()));
    }

    /// The recorded script text.
    #[must_use]
    pub fn script(&self) -> String {
        ScenarioScript {
            steps: self.steps.clone(),
        }
        .to_string()
    }
}

fn split(line: &str) -> (&str, &str) {
    match line.split_once(char::is_whitespace) {
        Some((command, argument)) => (command, argument.trim_start()),
        None => (line, ""),
    }
}

fn required(line: usize, command: &str, argument: &str) -> Result<String, ScriptError> {
    if argument.is_empty() {
        Err(ScriptError::at_line(
            line,
            format!("{command} requires an argument"),
        ))
    } else {
        Ok(argument.to_string())
    }
}

fn parse_number<T>(line: usize, command: &str, argument: &str) -> Result<T, ScriptError>
where
    T: std::str::FromStr,
{
    argument.parse().map_err(|_| {
        ScriptError::at_line(
            line,
            format!("{command} requires a number, found {argument:?}"),
        )
    })
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Recorder, ScenarioScript, ScriptEnv};
    use crate::controls::TextFieldEvent;
    use crate::test_support::fixtures::{
        ConfirmationSheetFixture, DocumentWindowFixture, TextFieldFixture,
    };
    use crate::test_support::scenario::Scenario;
    use std::time::Duration;

    #[test]
    fn records_and_replays_a_text_field_scenario() {
        let mut scenario = Scenario::new();
        let window = scenario.open(TextFieldFixture::new);
        let fixture = window.entity(scenario.app()).expect("text field fixture");
        let focus = scenario.update(|_, cx| fixture.read(cx).focus());

        let mut recorder = Recorder::new(&mut scenario);
        recorder.focus("field", &focus);
        recorder.type_text("hello");
        recorder.press("enter");
        recorder.advance(Duration::from_millis(16));
        recorder.expect_focus("field", &focus);
        recorder.expect_selector("field", "fixture-text-field");
        recorder.expect_window_count(1);
        let script_text = recorder.script();
        drop(recorder);

        assert_eq!(
            script_text,
            "focus field\ntype hello\npress enter\nadvance 16\nexpect-focus field\n\
             expect-selector field\nexpect-window-count 1\n"
        );
        assert_eq!(scenario.update(|_, cx| fixture.read(cx).text(cx)), "hello");

        let mut replay = Scenario::new();
        let window = replay.open(TextFieldFixture::new);
        let fixture = window
            .entity(replay.app())
            .expect("replayed text field fixture");
        let focus = replay.update(|_, cx| fixture.read(cx).focus());
        let mut env = ScriptEnv::new();
        env.bind_focus("field", focus)
            .bind_selector("field", "fixture-text-field");
        let script = ScenarioScript::parse(&script_text).expect("parse script");
        script.replay(&mut replay, &env).expect("replay script");

        assert_eq!(replay.update(|_, cx| fixture.read(cx).text(cx)), "hello");
        assert!(replay.update(|_, cx| fixture
            .read(cx)
            .events()
            .contains(&TextFieldEvent::Submitted)));
    }

    #[test]
    fn parse_and_replay_report_actionable_errors() {
        assert!(ScenarioScript::parse("bogus thing").is_err());

        let script = ScenarioScript::parse("focus missing").expect("parse script");
        let mut scenario = Scenario::new();
        let error = script
            .replay(&mut scenario, &ScriptEnv::new())
            .expect_err("missing focus binding");
        assert!(error.to_string().contains("no focus handle"));
    }

    #[test]
    fn script_activates_windows_and_checks_title() {
        let mut scenario = Scenario::new();
        scenario.open(|window, cx| DocumentWindowFixture::new(window, cx, "First"));
        scenario.open(|window, cx| DocumentWindowFixture::new(window, cx, "Second"));

        let script = ScenarioScript::parse(
            "expect-no-focus\nactivate 0\nexpect-title First\nexpect-window-count 2\n",
        )
        .expect("parse script");
        script
            .replay(&mut scenario, &ScriptEnv::new())
            .expect("replay script");
    }

    #[test]
    fn script_clicks_a_sheet_button() {
        let mut scenario = Scenario::new();
        let window = scenario.open(ConfirmationSheetFixture::new);
        let fixture = window.entity(scenario.app()).expect("sheet fixture");
        let mut env = ScriptEnv::new();
        env.bind_selector("accept", "sheet-accept");

        let script =
            ScenarioScript::parse("click accept\nexpect-window-count 1\n").expect("parse script");
        script.replay(&mut scenario, &env).expect("replay script");
        assert_eq!(
            scenario.update(|_, cx| fixture.read(cx).responses()),
            vec![true]
        );
    }
}
