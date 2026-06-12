use ratatui::text::{Line};

pub struct MessageBox<'a> {
   pub width: usize,
   pub lines: usize,
   pub scroll_amount: usize,
   pub text: Vec<Line<'a>>
}

impl<'a>  MessageBox<'a> {
   pub fn new(width: usize) -> Self {
      Self {
         width,
         lines: 0,
         scroll_amount: 0,
         text: Vec::new(),
      }
   }

   pub fn new_msg(&mut self, msg: &String) {
      let mut msg = Self::wrap_line(self.width, msg.clone());
      self.text.append(&mut msg.0);
      self.lines += msg.1;
   }

   pub fn wrap_lines(&mut self, width: usize, regular_msgs: &Vec<Line>) -> usize {
      let mut fixed_lines: Vec<Line> = Vec::new();
      self.lines = 0;
      for message in regular_msgs {
         let mut fixed = Self::wrap_line(width, message.to_string().clone());
         fixed_lines.append(&mut fixed.0);
         self.lines += fixed.1;
      }
      self.text = fixed_lines;
      self.width = width;
      self.lines
   }


   pub fn calculate_lines(&mut self, width: usize, regular_msgs: &Vec<Line>) {
      let new_line_count = self.wrap_lines(width, regular_msgs);
      if self.scroll_amount > new_line_count {
         self.scroll_amount -= new_line_count;
      }
   }

   pub fn wrap_msg(width: usize, mut msg: String) -> (String, usize) {
      let mut lines_added = 0;
      let mut trimmed_msgs: Vec<String> = Vec::new();
      let mut split_msg;
      while msg.trim().len() >= width {
         lines_added += 1;
         let cloned = msg.clone();
         split_msg = cloned.split_at(width);
         msg = split_msg.1.to_string();
         trimmed_msgs.push(split_msg.0.to_string());
      }
      trimmed_msgs.push(msg);
      lines_added += 1;

      (trimmed_msgs.join("\n"), lines_added)
   }

   pub fn wrap_line(width: usize, mut msg: String) -> (Vec<Line<'a>>, usize) {
      let mut lines_added = 0;
      let mut trimmed_msgs: Vec<Line> = Vec::new();
      let mut split_msg;
      while msg.trim().len() >= width {
         lines_added += 1;
         let cloned = msg.clone();
         split_msg = cloned.split_at(width);
         msg = split_msg.1.to_string();
         trimmed_msgs.push(Line::raw(split_msg.0.to_string()));
      }
      trimmed_msgs.push(Line::raw(msg));
      lines_added += 1;
      
      (trimmed_msgs, lines_added)
   }
}
