use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, ToLine};

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

      let mut bold = false;

      let mut cur_span = Span::default();

      loop {
         if let Some(ch) = iter.next() {
            if ch.eq(&'*') {
               spans.push(cur_span.clone());
               cur_span = Span::default();
               bold = !bold;
               continue;
            }

            if bold {
               let mut content = cur_span.content.to_string();
               content.push(ch);
               cur_span = cur_span.content(content).red();
            } else {
               let mut content = cur_span.content.to_string();
               content.push(ch);
               cur_span = cur_span.content(content);
            }
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

   pub fn wrap_line(width: usize, mut line: &Line<'a>) -> (Vec<Line<'a>>, usize)  {
      let new_line = line.clone();

      let mut finished = Vec::new();

      let mut current_line = Line::default();
      let mut current_span_len;
      let mut current_span_style = Style::default();
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

   fn append_span_to_line(span: Span<'a>, current_line: Line<'a>, width: usize) -> (Vec<Line<'a>>, usize)  {
      let span_length = span.content.len();
      let line_length = current_line.to_string().len();
      let mut new_lines = Vec::new();
      let span_style = span.style;
      let mut line_content = current_line.to_string();
      line_content.push_str(&span.content.to_string());
      let mut new_lines_added = 1;

      if !Self::fits_in_box(width, line_length, span_length) {
         let (content, new_line_content) = line_content.split_at(width);
         new_lines.push(Line::from(Span::from(content.to_string()).style(span_style)));
         let new_span = Span::from(new_line_content.to_string());
         let results = &mut Self::append_span_to_line(new_span, Line::default(), width);
         new_lines.append(&mut results.0);
         new_lines_added += results.1;
      } else {
         new_lines.push(Line::from(Span::from(line_content).style(span_style)));
      }

      (new_lines, new_lines_added)
   }

   pub fn fits_in_box(width: usize, current_size: usize, to_be_added: usize) -> bool {
      current_size + to_be_added <= width
   }
}
