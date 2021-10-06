use crate::database::Entry;
use core::fmt;
use serde::{Serialize, Serializer};

use pointplots::{Chart, PixelColor, Plot, Point, Shape};

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

use tera::{Context, Tera};

#[derive(Debug, Clone, PartialEq)]
struct Timestamp(f64);

impl From<f64> for Timestamp {
  fn from(number: f64) -> Self {
    Timestamp(number)
  }
}

impl From<Timestamp> for f64 {
  fn from(timestamp: Timestamp) -> Self {
    timestamp.0
  }
}

impl From<&Timestamp> for f64 {
  fn from(timestamp: &Timestamp) -> Self {
    timestamp.0
  }
}

impl<T: TimeZone> From<DateTime<T>> for Timestamp {
  fn from(date: DateTime<T>) -> Self {
    Timestamp(date.timestamp() as f64)
  }
}

impl fmt::Display for Timestamp {
  fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), std::fmt::Error> {
    let date = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(self.0 as i64, 0), Utc);
    f.write_fmt(format_args!("{}", date.format("%Y-%m-%d")))
  }
}

impl Serialize for Timestamp {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&format!("{}", &self))
  }
}

impl Entry {
  /// Calculates a Deck's total score based on the score of the list done vs the other lists.
  /// Ex:
  /// ```
  /// use card_counter::{database::Entry, score::Deck};
  /// let entry = Entry {
  ///       board_id: "board-id-1".to_string(),
  ///       time_stamp: 1,
  ///       decks: vec![
  ///         Deck {list_name: "listA".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "listB".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "Done".to_string(), size: 10, score: 40, unscored: 0, estimated: 40 }
  ///       ],
  ///   };
  ///
  /// assert_eq!((40, 40), entry.calculate_score(&None));
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

#[derive(Debug, PartialEq)]
pub struct Burndown(pub Vec<(DateTime<Utc>, i32, i32)>);

impl Burndown {
  /// Aggregates the score of a set of entries into a list of 3-tuples
  /// of [("dd-mm-yyyy", i32, i32)...] for ease in rendering content
  /// to a human useable form.
  /// Ex:
  /// ```
  /// use card_counter::{database::Entry, score::Deck, commands::burndown::Burndown};
  /// use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
  /// let entry = Entry {
  ///       board_id: "board-id-1".to_string(),
  ///       time_stamp: 1,
  ///       decks: vec![
  ///         Deck {list_name: "listA".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "listB".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "Done".to_string(), size: 10, score: 40, unscored: 0, estimated: 40 }
  ///       ],
  ///   };
  /// let entry2 = Entry {
  ///       board_id: "board-id-1".to_string(),
  ///       time_stamp: 86401,
  ///       decks: vec![
  ///         Deck {list_name: "listA".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "listB".to_string(), size: 5, score: 10, unscored: 0, estimated: 10 },
  ///         Deck {list_name: "Done".to_string(), size: 10, score: 50, unscored: 0, estimated: 50 }
  ///       ],
  ///   };
  /// let entries = vec![entry, entry2];
  /// let timestamp = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1, 0), Utc);
  /// let timestamp2 = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(86401, 0), Utc);
  /// assert_eq!(vec![(timestamp, 40, 40), (timestamp2, 30, 50)], Burndown::calculate_burndown(&entries, &None).0);
  /// ```
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

  /// Formats a Burndown struct as a vector of csv, with the first row being the header row.
  /// Ex:
  /// ```
  /// use card_counter::{database::Entry, score::Deck, commands::burndown::Burndown};
  /// use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
  /// let entry = Entry {
  ///       board_id: "board-id-1".to_string(),
  ///       time_stamp: 1,
  ///       decks: vec![
  ///         Deck {list_name: "listA".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "listB".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "Done".to_string(), size: 10, score: 40, unscored: 0, estimated: 40 }
  ///       ],
  ///   };
  /// let entry2 = Entry {
  ///       board_id: "board-id-1".to_string(),
  ///       time_stamp: 86401,
  ///       decks: vec![
  ///         Deck {list_name: "listA".to_string(), size: 5, score: 20, unscored: 0, estimated: 20 },
  ///         Deck {list_name: "listB".to_string(), size: 5, score: 10, unscored: 0, estimated: 10 },
  ///         Deck {list_name: "Done".to_string(), size: 10, score: 50, unscored: 0, estimated: 50 }
  ///       ],
  ///   };
  /// let entries = vec![entry, entry2];
  /// let timestamp = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1, 0), Utc);
  /// let timestamp2 = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(86401, 0), Utc);
  /// assert_eq!(vec!["Date,Incomplete,Complete", "01-01-1970,40,40", "02-01-1970,30,50"], Burndown::calculate_burndown(&entries, &None).as_csv());
  ///```
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

  /// Generates an ASCII graph of the Burndown struct and prints it to standard out
  pub fn as_ascii(&self) -> Result<(), ()> {
    let start_date: DateTime<Utc> = self.0.first().unwrap().0;
    let end_date: DateTime<Utc> = self.0.last().unwrap().0;

    let max_complete: i32 = self.max_complete();

    let max_incomplete: i32 = self.max_incomplete();

    let max_y = max_complete.max(max_incomplete) as f64;

    let incomplete: Vec<Point<Timestamp, f64>> = self.incomplete_as_points();

    let complete: Vec<Point<Timestamp, f64>> = self.complete_as_points();

    println!("Max: {}", max_y);
    println!("\nBurndown Chart\n");
    Chart::new(
      120,
      60,
      start_date.timestamp() as f64,
      end_date.timestamp() as f64,
    )
    .lineplot_with_tags(
      &Shape::Lines(&complete),
      Some("Complete".to_string()),
      PixelColor::Blue,
    )
    .lineplot_with_tags(
      &Shape::Lines(&incomplete),
      Some("Incomplete".to_string()),
      PixelColor::Red,
    )
    .display();

    Ok(())
  }

  /// Generates an SVG graph of the Burndown struct and prints it to standard out
  pub fn as_svg(&self) -> Result<String, ()> {
    let mut context = Context::new();

    //hardset the padding around the graph
    let padding = 50;

    //ensure the viewbox is as per input
    let width = 900 - padding * 2;
    let height = 600 - padding * 2;

    let max_complete: i32 = self.max_complete();
    let max_incomplete: i32 = self.max_incomplete();

    let max_y: f64 = max_complete.max(max_incomplete).into();
    let min_x = self.min_date().timestamp() as f64;
    let max_x = self.max_date().timestamp() as f64;

    let point_to_path = |index: usize, point: &Point<Timestamp, f64>| -> String {
      let x = (f64::from(&point.x) - min_x) / (max_x - min_x) * width as f64 + padding as f64;
      let y = point.y / max_y * (height as f64 * -1.0) + height as f64 + padding as f64;
      if index == 0 {
        format!("M {} {}", x, y)
      } else {
        format!("L {} {}", x, y)
      }
    };

    let incomplete_path = &self
      .incomplete_as_points()
      .iter()
      .enumerate()
      .map(|(i, path)| point_to_path(i, path))
      .collect::<Vec<String>>()
      .join(" ");

    let complete_path = self
      .complete_as_points()
      .iter()
      .enumerate()
      .map(|(i, path)| point_to_path(i, path))
      .collect::<Vec<String>>()
      .join(" ");

    context.insert("name", "Burndown");
    context.insert("width", &width);
    context.insert("height", &height);
    context.insert("padding", &padding);
    context.insert("incomplete_path", &incomplete_path);
    context.insert("incomplete_colour", "#D2222D");
    context.insert("complete_path", &complete_path);
    context.insert("complete_colour", "#238823");
    context.insert("max_y", &max_y);
    context.insert("y_labels", &[0., (max_y / 2.).round(), max_y]);

    let mid_date = (max_x - min_x) / 2. + min_x;
    context.insert(
      "x_labels",
      &[
        Timestamp::from(min_x),
        Timestamp::from(mid_date),
        Timestamp::from(max_x),
      ],
    );

    let graph = Tera::one_off(include_str!("../template/burndown.svg"), &context, true)
      .expect("Could not draw graph");
    Ok(graph)
  }

  /// Returns the date with the highest value
  fn max_date(&self) -> DateTime<Utc> {
    *self.0.iter().map(|(date, _, _)| date).max().unwrap()
  }

  /// Returns the date with the lowest value
  fn min_date(&self) -> DateTime<Utc> {
    *self.0.iter().map(|(date, _, _)| date).min().unwrap()
  }

  /// Returns the highest score from the complete category
  fn max_complete(&self) -> i32 {
    *self
      .0
      .iter()
      .map(|(_, _, completed)| completed)
      .max()
      .unwrap()
  }

  /// Returns the highest score from the incomplete category
  fn max_incomplete(&self) -> i32 {
    *self
      .0
      .iter()
      .map(|(_, incompleted, _)| incompleted)
      .max()
      .unwrap()
  }

  /// Extracts the incomplete and date scores and maps them into a Vec
  /// of pointplots::Point struct.
  fn incomplete_as_points(&self) -> Vec<Point<Timestamp, f64>> {
    self
      .0
      .iter()
      .map(|(date, incompleted, _)| -> Point<Timestamp, f64> {
        {
          Point {
            x: date.to_owned().into(),
            y: *incompleted as f64,
          }
        }
      })
      .collect()
  }

  /// Extracts the complete and date scores and maps them into a Vec
  /// of pointplots::Point struct.
  fn complete_as_points(&self) -> Vec<Point<Timestamp, f64>> {
    self
      .0
      .iter()
      .map(|(date, _, complete)| -> Point<Timestamp, f64> {
        {
          Point {
            x: date.to_owned().into(),
            y: *complete as f64,
          }
        }
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use crate::{commands::burndown::*, database::Entry, score::Deck};
  fn gen_burndown() -> Burndown {
    let entries = vec![
      Entry {
        board_id: "board-id-1".to_string(),
        time_stamp: 1,
        decks: vec![
          Deck {
            list_name: "listA".to_string(),
            size: 5,
            score: 20,
            unscored: 0,
            estimated: 20,
          },
          Deck {
            list_name: "listB".to_string(),
            size: 5,
            score: 20,
            unscored: 0,
            estimated: 20,
          },
          Deck {
            list_name: "Done".to_string(),
            size: 10,
            score: 40,
            unscored: 0,
            estimated: 40,
          },
        ],
      },
      Entry {
        board_id: "board-id-1".to_string(),
        time_stamp: 43200,
        decks: vec![
          Deck {
            list_name: "listA".to_string(),
            size: 5,
            score: 20,
            unscored: 0,
            estimated: 20,
          },
          Deck {
            list_name: "listB".to_string(),
            size: 5,
            score: 20,
            unscored: 0,
            estimated: 20,
          },
          Deck {
            list_name: "Done".to_string(),
            size: 10,
            score: 40,
            unscored: 0,
            estimated: 40,
          },
        ],
      },
      Entry {
        board_id: "board-id-1".to_string(),
        time_stamp: 86401,
        decks: vec![
          Deck {
            list_name: "listA".to_string(),
            size: 5,
            score: 20,
            unscored: 0,
            estimated: 20,
          },
          Deck {
            list_name: "listB".to_string(),
            size: 5,
            score: 10,
            unscored: 0,
            estimated: 10,
          },
          Deck {
            list_name: "Done".to_string(),
            size: 10,
            score: 50,
            unscored: 0,
            estimated: 50,
          },
        ],
      },
    ];

    Burndown::calculate_burndown(&entries, &None)
  }

  #[test]
  fn it_calculates_max_date() {
    assert_eq!(gen_burndown().max_date().timestamp(), 86401)
  }

  #[test]
  fn it_calculates_min_date() {
    assert_eq!(gen_burndown().min_date().timestamp(), 1)
  }

  #[test]
  fn it_returns_max_completed() {
    assert_eq!(gen_burndown().max_complete(), 50)
  }

  #[test]
  fn it_returns_max_incompleted() {
    assert_eq!(gen_burndown().max_incomplete(), 40)
  }

  #[test]
  fn it_returns_completed_as_points() {
    assert_eq!(
      gen_burndown().complete_as_points(),
      vec![
        Point {
          x: Timestamp(1.0),
          y: 40.0
        },
        Point {
          x: Timestamp(43200.0),
          y: 40.0
        },
        Point {
          x: Timestamp(86401.0),
          y: 50.0
        }
      ]
    )
  }
  #[test]
  fn it_returns_incompleted_as_points() {
    assert_eq!(
      gen_burndown().incomplete_as_points(),
      vec![
        Point {
          x: Timestamp(1.0),
          y: 40.0
        },
        Point {
          x: Timestamp(43200.0),
          y: 40.0
        },
        Point {
          x: Timestamp(86401.0),
          y: 30.0
        }
      ]
    )
  }
}
