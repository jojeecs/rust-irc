pub struct MessageBox {
   pub messages: Vec<String>,
   pub width: usize,
   pub lines: usize,
   pub scroll_amount: usize,
}

impl MessageBox {
   pub fn new(width: usize) -> Self {
      Self {
         messages: Vec::new(),
         width,
         lines: 0,
         scroll_amount: 0,
      }
   }

   pub fn new_msg(&mut self, msg: &String) {
      let msg = Self::wrap_msg(self.width, msg.clone());
      self.messages.push(msg.0);
      self.lines += msg.1;
   }

   pub fn wrap_msg(width: usize, mut msg: String) -> (String, usize) {
      let mut lines_added = 0;
      let mut trimmed_msgs: Vec<String> = Vec::new();
      let mut split_msg;
      while msg.trim().len() >= width - 2 {
         lines_added += 1;
         let cloned = msg.clone();
         split_msg = cloned.split_at(width - 2);
         msg = split_msg.1.to_string();
         trimmed_msgs.push(split_msg.0.to_string());
      }

      trimmed_msgs.push(msg);
      lines_added += 1;
      
      (trimmed_msgs.join("\n"), lines_added)
   }
}
