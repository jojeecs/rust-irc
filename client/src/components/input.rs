use sha3::{Digest, Sha3_256};
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

    pub fn new(label: String) -> Self {
        let mut new = Self::default();
        new.label = label;
        new
    }
    pub fn is_empty(&self) -> bool {
        self.input.value().is_empty()
    }

    pub fn display(&self) -> String {
        if self.hidden {
            return "*".repeat(self.input.to_string().len());
        }
        self.input.to_string()
    }

    pub fn value(&self) -> String {
        if self.hidden {
            let mut hasher = Sha3_256::new();
            let pass = self.input.to_string();
            hasher.update(pass);
            let hash = hasher.finalize();

            let mut password_hash = String::new();

            for byte in hash {
                password_hash.push_str(&format!("{:02x}", byte));
            }
            return password_hash
        }

        self.input.to_string()
    }

    pub fn set_hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

}