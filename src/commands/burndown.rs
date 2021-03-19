use std::{fs::File, str::FromStr};

use crate::{
  database::Entry,
  util::{PixelState, TextDrawingBackend},
};

use chrono::{DateTime, Datelike, NaiveDateTime, TimeZone, Utc};
use plotters::{prelude::*, style};

impl Entry {
  /// Calculates a Deck's total score based on the score of the list done vs the other lists.
  /// Ex:
  /// ```
  /// use crate::{database::Entry, score::Deck};
  /// let entry = Entry {
  ///       board_id: "board-id-1",
  ///       time_stamp: 1,
  ///       decks: vec![
  ///         Deck {list_name: "listA", size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "listB", size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "Done", size: 10, score: 40, unscored: 0, estimated: 40 }
  ///       ],
  ///   };
  ///
  /// assert((20, 20), entry.calculate_score(None));
  /// ```
  pub fn calculate_score(&self, filter: &Option<&str>) -> (i32, i32) {
    self
      .decks
      .iter()
      .fold((0, 0), |(incomplete, complete), deck| -> (i32, i32) {
        if filter.is_some() && deck.list_name.contains(filter.unwrap()) {
          (incomplete, complete)
        } else if deck.list_name.contains("Done") {
          (incomplete, complete + deck.score)
        } else {
          (incomplete + deck.score, complete)
        }
      })
  }
}

pub struct Burndown(Vec<(DateTime<Utc>, i32, i32)>);

impl Burndown {
  /// Aggregates the score of a set of entries into a list of 3-tuples
  /// of [("dd-mm-yyyy", i32, i32)...] for ease of in rendering
  /// content to a human useable form.
  pub fn calculate_burndown(entries: &[Entry], filter: &Option<&str>) -> Self {
    let mut entries = entries.to_vec();

    // In some cases, there are going to be multiple entries for a
    // single days when building a burndown chart, we want to use the
    // last entry in that day
    entries.sort();
    let mut burndown: Vec<(DateTime<Utc>, i32, i32)> = Vec::new();
    entries.into_iter().for_each(|entry| {
      let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(entry.time_stamp, 0), Utc);
      let (incomplete, complete) = entry.calculate_score(filter);

      // Remove duplicate entry
      if let Some(entry) = burndown.last() {
        if entry.0 == time {
          burndown.pop();
        }
      }

      burndown.push((time, incomplete, complete));
    });

    Burndown(burndown)
  }

  pub fn as_csv(&self) -> Vec<String> {
    let mut output = vec!["Date,Incomplete,Complete".to_string()];
    output.extend(self.0.iter().map(|(time, incomplete, complete)| {
      format!(
        "{},{},{}",
        time.format("%d-%m-%Y").to_string(),
        incomplete,
        complete
      )
    }));

    output
  }

  pub fn as_svg(&self) -> Result<(), ()> {
    let root_area = SVGBackend::new("images/burndown.svg", (600, 400)).into_drawing_area();
    root_area.fill(&style::WHITE).unwrap();

    let start_date = self.0.first().unwrap().0;
    let end_date = self.0.last().unwrap().0;
    let max_completed = self
      .0
      .iter()
      .map(|(_, completed, _)| completed)
      .max()
      .unwrap();
    let mut ctx = ChartBuilder::on(&root_area)
      .set_label_area_size(LabelAreaPosition::Left, 40)
      .set_label_area_size(LabelAreaPosition::Bottom, 40)
      .caption("Burndown Chart", ("sans-serif", 40))
      .build_cartesian_2d(start_date..end_date, 0..*max_completed)
      .unwrap();

    ctx.configure_mesh().draw().unwrap();

    ctx
      .draw_series(LineSeries::new(
        self
          .0
          .iter()
          .map(|(date, completed, _)| (*date, *completed)),
        &style::BLUE,
      ))
      .unwrap();

    ctx
      .draw_series(LineSeries::new(
        self
          .0
          .iter()
          .map(|(date, _, incomplete)| (*date, *incomplete)),
        &style::RED,
      ))
      .unwrap();

    Ok(())
  }

  pub fn as_ascii(&self) -> Result<(), ()> {
    let root_area = TextDrawingBackend(vec![PixelState::Empty; 10000]).into_drawing_area();
    root_area.fill(&style::WHITE).unwrap();

    let start_date: DateTime<Utc> = self.0.first().unwrap().0;
    let end_date: DateTime<Utc> = self.0.last().unwrap().0;
    let range = start_date..end_date;
    let max_complete: i32 = *self
      .0
      .iter()
      .map(|(_, completed, _)| completed)
      .max()
      .unwrap();

    let max_incomplete: i32 = *self
      .0
      .iter()
      .map(|(_, _, incomplete)| incomplete)
      .max()
      .unwrap();

    let max = max_complete.max(max_incomplete);
    println!("Max: {}", max);
    println!("Range: {:?}", range);
    let mut ctx = ChartBuilder::on(&root_area)
      .margin(1)
      .set_label_area_size(LabelAreaPosition::Left, (5i32).percent_width())
      .set_label_area_size(LabelAreaPosition::Bottom, (10i32).percent_height())
      .caption("Burndown Chart", ("sans-serif", (10).percent_height()))
      .build_cartesian_2d(range, 0..max)
      .unwrap();

    ctx
      .configure_mesh()
      .disable_x_mesh()
      .disable_y_mesh()
      .draw()
      .unwrap();

    let date_completed = self
      .0
      .iter()
      .map(|(date, completed, _)| (*date, *completed));

    ctx
      .draw_series(LineSeries::new(
        date_completed.clone(),
        &style::BLUE,
      ))
      .unwrap();

    let date_incompleted = self
      .0
      .iter()
      .map(|(date, _, incomplete)| (*date, *incomplete));

    ctx
      .draw_series(LineSeries::new(
        date_incompleted.clone(),
        &style::RED,
      ))
      .unwrap();
    root_area.present().unwrap();
    Ok(())
  }

  pub fn test() -> Result<(), ()> {
    test_chart(TextDrawingBackend((50, 100)).into_drawing_area()).unwrap();
    Ok(())
  }
}

fn test_chart<DB: DrawingBackend>(b: DrawingArea<DB, plotters::coord::Shift>) -> Result<(), ()> {
  let complete = File::open("test.json").unwrap();
  let incomplete = File::open("test2.json").unwrap();

  let completed: Vec<(DateTime<Utc>, i32)> = serde_json::from_reader(complete).unwrap();
  let incompleted: Vec<(DateTime<Utc>, i32)> = serde_json::from_reader(incomplete).unwrap();
  let date_range = completed.first().unwrap().0..completed.last().unwrap().0;
  println!("Range: {:?}", date_range);
  let mut chart = ChartBuilder::on(&b)
    .margin(1)
    .caption("Burndown", ("sans-serif", (10).percent_height()))
    .set_label_area_size(LabelAreaPosition::Left, (10i32).percent_width())
    .set_label_area_size(LabelAreaPosition::Bottom, (10i32).percent_height())
    .build_cartesian_2d(date_range, 0..100)
    .unwrap();

  chart
    .configure_mesh()
    .disable_x_mesh()
    .disable_y_mesh()
    .draw()
    .unwrap();

  chart
    .draw_series(LineSeries::new(
      completed.into_iter(),
      &RED,
    ))
    .unwrap();

  chart
    .draw_series(LineSeries::new(
      incompleted.into_iter(),
      &RED,
    ))
    .unwrap();

  b.present().unwrap();

  Ok(())
}
