use plotters::prelude::*;
use plotters::style::text_anchor::{HPos, VPos};
use plotters_backend::{
  BackendColor, BackendStyle, BackendTextStyle, DrawingBackend, DrawingErrorKind,
};
use std::error::Error;

#[derive(Copy, Clone)]
pub enum PixelState {
  Empty,
  HLine,
  VLine,
  Cross,
  Pixel,
  Text(char),
  Circle(bool),
}

impl PixelState {
  pub fn to_char(self) -> char {
    match self {
      Self::Empty => ' ',
      Self::HLine => '-',
      Self::VLine => '|',
      Self::Cross => '+',
      Self::Pixel => '.',
      Self::Text(c) => c,
      Self::Circle(filled) => {
        if filled {
          '@'
        } else {
          'O'
        }
      }
    }
  }

  pub fn update(&mut self, new_state: PixelState) {
    let next_state = match (*self, new_state) {
      (Self::HLine, Self::VLine) => Self::Cross,
      (Self::VLine, Self::HLine) => Self::Cross,
      (_, Self::Circle(what)) => Self::Circle(what),
      (Self::Circle(what), _) => Self::Circle(what),
      (_, Self::Pixel) => Self::Pixel,
      (Self::Pixel, _) => Self::Pixel,
      (_, new) => new,
    };

    *self = next_state;
  }
}

pub struct TextDrawingBackend {
  size: (u32, u32),
  buffer: Vec<PixelState>
}

impl TextDrawingBackend {
  pub fn new(size: (u32, u32)) -> Self{
    let buffer_size = (size.0 * size.1) as usize;
   TextDrawingBackend{
     size,
     buffer: vec![PixelState::Empty; buffer_size]
   }
  }
}
impl DrawingBackend for TextDrawingBackend {
  type ErrorType = std::io::Error;

  fn get_size(&self) -> (u32, u32) {
    self.size
  }

  fn ensure_prepared(&mut self) -> Result<(), DrawingErrorKind<std::io::Error>> {
    Ok(())
  }

  fn present(&mut self) -> Result<(), DrawingErrorKind<std::io::Error>> {
    for r in 0..30 {
      let mut buf = String::new();
      for c in 0..100 {
        buf.push(self.buffer[r * 100 + c].to_char());
      }
      println!("{}", buf);
    }

    Ok(())
  }

  fn draw_pixel(
    &mut self,
    pos: (i32, i32),
    color: BackendColor,
  ) -> Result<(), DrawingErrorKind<std::io::Error>> {
    if color.alpha > 0.3 {
      self.buffer[(pos.1 * 100 + pos.0) as usize].update(PixelState::Pixel);
    }
    Ok(())
  }

  fn draw_line<S: BackendStyle>(
    &mut self,
    from: (i32, i32),
    to: (i32, i32),
    style: &S,
  ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
    if from.0 == to.0 {
      let x = from.0;
      let y0 = from.1.min(to.1);
      let y1 = from.1.max(to.1);
      for y in y0..y1 {
        self.buffer[(y * 100 + x) as usize].update(PixelState::VLine);
      }
      return Ok(());
    }

    if from.1 == to.1 {
      let y = from.1;
      let x0 = from.0.min(to.0);
      let x1 = from.0.max(to.0);
      for x in x0..x1 {
        self.buffer[(y * 100 + x) as usize].update(PixelState::HLine);
      }
      return Ok(());
    }

    plotters_backend::rasterizer::draw_line(self, from, to, style)
  }

  fn estimate_text_size<S: BackendTextStyle>(
    &self,
    text: &str,
    _: &S,
  ) -> Result<(u32, u32), DrawingErrorKind<Self::ErrorType>> {
    Ok((text.len() as u32, 1))
  }

  fn draw_text<S: BackendTextStyle>(
    &mut self,
    text: &str,
    style: &S,
    pos: (i32, i32),
  ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
    let (width, height) = self.estimate_text_size(text, style)?;
    let (width, height) = (width as i32, height as i32);
    let dx = match style.anchor().h_pos {
      HPos::Left => 0,
      HPos::Right => -width,
      HPos::Center => -width / 2,
    };
    let dy = match style.anchor().v_pos {
      VPos::Top => 0,
      VPos::Center => -height / 2,
      VPos::Bottom => -height,
    };
    let offset = (pos.1 + dy).max(0) * 100 + (pos.0 + dx).max(0);
    for (idx, chr) in (offset..).zip(text.chars()) {
      self.buffer[idx as usize].update(PixelState::Text(chr));
    }
    Ok(())
  }
}
