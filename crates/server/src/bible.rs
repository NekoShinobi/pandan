use chrono::{NaiveDate, Utc};
use serde_json::{Value, json};
use std::sync::OnceLock;

const ENGLISH_REVISED_VERSION: &str = include_str!("../../../data/english-revised-version.json");
const TRANSLATION: &str = "English Revised Version";

static VERSES: OnceLock<Result<Vec<Verse>, String>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct Verse {
    reference: String,
    text: String,
}

pub async fn daily_verse() -> Result<Value, String> {
    tokio::task::spawn_blocking(|| daily_verse_for(Utc::now().date_naive()))
        .await
        .map_err(|_| "daily verse selection was interrupted".to_owned())?
}

fn daily_verse_for(date: NaiveDate) -> Result<Value, String> {
    let verses = match VERSES.get_or_init(parse_verses) {
        Ok(verses) => verses,
        Err(error) => return Err(error.clone()),
    };
    if verses.is_empty() {
        return Err("the bundled Bible contains no verses".to_owned());
    }

    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid Unix epoch date");
    let day = date.signed_duration_since(epoch).num_days();
    let index = day.rem_euclid(verses.len() as i64) as usize;
    let verse = &verses[index];

    Ok(json!({
        "items": [{
            "title": verse.text,
            "source": verse.reference,
            "version": TRANSLATION,
            "published_at": date.to_string(),
        }]
    }))
}

fn parse_verses() -> Result<Vec<Verse>, String> {
    let bible: Value = serde_json::from_str(ENGLISH_REVISED_VERSION)
        .map_err(|_| "the bundled Bible data is invalid".to_owned())?;
    let books = bible
        .as_object()
        .ok_or_else(|| "the bundled Bible data is invalid".to_owned())?;
    let mut book_names = books.keys().collect::<Vec<_>>();
    book_names.sort_unstable();

    let mut verses = Vec::new();
    for book_name in book_names {
        let chapters = books[book_name]
            .as_object()
            .ok_or_else(|| "the bundled Bible data is invalid".to_owned())?;
        let mut chapter_numbers = chapters
            .keys()
            .filter_map(|chapter| chapter.parse::<u16>().ok())
            .collect::<Vec<_>>();
        chapter_numbers.sort_unstable();

        for chapter_number in chapter_numbers {
            let chapter_key = chapter_number.to_string();
            let chapter = chapters[&chapter_key]
                .as_object()
                .ok_or_else(|| "the bundled Bible data is invalid".to_owned())?;
            let mut verse_numbers = chapter
                .keys()
                .filter_map(|verse| verse.parse::<u16>().ok())
                .collect::<Vec<_>>();
            verse_numbers.sort_unstable();

            for verse_number in verse_numbers {
                let verse_key = verse_number.to_string();
                let text = chapter[&verse_key]
                    .as_str()
                    .ok_or_else(|| "the bundled Bible data is invalid".to_owned())?;
                verses.push(Verse {
                    reference: format!("{book_name} {chapter_number}:{verse_number}"),
                    text: text.to_owned(),
                });
            }
        }
    }
    Ok(verses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_bundled_translation() {
        let verses = parse_verses().expect("Bible data should parse");
        assert!(verses.len() > 30_000);
        assert!(verses.iter().any(|verse| {
            verse.reference == "Genesis 1:1"
                && verse.text == "In the beginning God created the heaven and the earth."
        }));
    }

    #[test]
    fn selection_is_stable_for_a_calendar_day() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 15).expect("valid date");
        assert_eq!(daily_verse_for(date), daily_verse_for(date));
    }

    #[test]
    fn selection_advances_each_day() {
        let first = NaiveDate::from_ymd_opt(2026, 8, 15).expect("valid date");
        let second = first.succ_opt().expect("next day");
        let first_reference = daily_verse_for(first).expect("daily verse")["items"][0]["source"]
            .as_str()
            .expect("verse reference")
            .to_owned();
        let second_reference = daily_verse_for(second).expect("daily verse")["items"][0]["source"]
            .as_str()
            .expect("verse reference")
            .to_owned();
        assert_ne!(first_reference, second_reference);
    }
}
