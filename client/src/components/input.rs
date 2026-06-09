use tui_input::Input;


pub struct InputField {
    pub label: String,
    pub input: Input,
    pub focused: bool,
    pub hidden: bool,
}

impl Default for InputField {
    fn default() -> Self {
        Self {
            label: String::default(),
            input: Input::default(),
            focused: false,
            hidden: false,
        }
    }
}

impl InputField {
    pub fn is_empty(&self) -> bool {
        return self.input.value().is_empty()
    }

    pub fn value(&self) -> String {
        if self.hidden {
            return "*".repeat(self.input.to_string().len());
        }
        self.input.to_string()
    }

    pub fn set_hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

}