use ratatui::style::{Style};
use ratatui::text::{Line, Span};

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
      let spans = Self::string_to_spans(msg.clone());
      let line = Line::from(spans);
      let mut msg = Self::wrap_line(self.width, &line);
      self.text.append(&mut msg.0);
      self.lines += msg.1;
   }

   pub fn wrap_lines(&mut self, width: usize, regular_msgs: &Vec<Line<'a>>) -> usize {
      let mut fixed_lines: Vec<Line> = Vec::new();
      self.lines = 0;
      for message in regular_msgs {
         let spans  = Self::string_to_spans(message.to_string());
         let line = Line::from(spans);
         let mut fixed = Self::wrap_line(width, &line);
         fixed_lines.append(&mut fixed.0);
         self.lines += fixed.1;
      }
      self.text = fixed_lines;
      self.width = width;
      self.lines
   }

   fn string_to_spans(msg: String) -> Vec<Span<'a>> {
      let mut spans = Vec::new();

      let mut iter = msg.chars().into_iter();

      let mut styled = false;
      let mut style = Style::default();

      let mut cur_span = Span::default();

      let id_string = ['~'];

      let mut current_char;

      loop {
         if let Some(ch) = iter.next() {
            current_char = ch;
            if id_string.contains(&current_char) {
               spans.push(cur_span.clone());
               cur_span = Span::default();
               styled = !styled;
               match ch {
                  '~' => {
                     if let Some(char) = iter.next() {
                        current_char = char;
                        match current_char {
                           'r' => {
                              style = Style::default().red();
                              continue;
                           },
                           'b' => {
                              style = Style::default().red();
                           }
                           _ => {
                              style = Style::default();
                              continue;
                           }
                        };
                     } else {
                        continue;
                     }
                  }
                  _ => {}
               }
            }

            if !styled {
               style = Style::default();
            }

            let mut content = cur_span.content.to_string();
            content.push(current_char);
            cur_span = cur_span.content(content).style(style);
         } else {
            spans.push(cur_span.clone());
            break;
         }
      }

      spans

   }

   pub fn calculate_lines(&mut self, width: usize, regular_msgs: &Vec<Line<'a>>) {
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

   pub fn wrap_line(width: usize, line: &Line<'a>) -> (Vec<Line<'a>>, usize)  {
      let new_line = line.clone();

      let mut finished = Vec::new();

      let mut current_line = Line::default();
      let mut current_span_len;
      let mut current_span_style;
      let mut current_line_len = current_line.to_string().len();

      for span in new_line {
         current_span_len = span.content.len();
         current_span_style = span.style;
         if Self::fits_in_box(width, current_line_len, current_span_len) {
            current_line.push_span(span.clone());
            current_line_len = current_line.to_string().len();
         } else {
            for word in span.content.split("") {
               let new_word_len = word.len();
               if Self::fits_in_box(width, current_line_len, new_word_len) {
                  current_line.push_span(Span::from(word.to_string()).style(current_span_style));
                  current_line_len = current_line.to_string().len();
               } else {
                  finished.push(current_line.clone());
                  current_line = Line::default();
                  current_line_len = 0;
               }
            }
         }
      }

      finished.push(current_line);
      let lines_added = finished.len();

      (finished, lines_added)
   }

   pub fn fits_in_box(width: usize, current_size: usize, to_be_added: usize) -> bool {
      current_size + to_be_added <= width
   }
}
