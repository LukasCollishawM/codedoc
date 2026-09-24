use proptest::prelude::*;
use rusqlite::Connection;

fn searchable() -> Connection {
    let connection = Connection::open_in_memory().expect("an in-memory database");
    connection
        .execute_batch(
            "CREATE VIRTUAL TABLE claims USING fts5(
                 record_id UNINDEXED,
                 claim,
                 detail,
                 tokenize = 'unicode61 remove_diacritics 2'
             );
             INSERT INTO claims (record_id, claim, detail)
             VALUES ('a', 'Tenant isolation depends on validating the token first', 'retry-after');",
        )
        .expect("the table is created");
    connection
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn no_query_a_person_can_type_reaches_the_full_text_parser_as_syntax(raw in ".{0,120}") {
        let connection = searchable();
        let expression = codedoc_index::fts_expression(&raw);
        if expression.is_empty() {
            return Ok(());
        }
        let outcome: Result<i64, _> = connection.query_row(
            "SELECT count(*) FROM claims WHERE claims MATCH ?1",
            [&expression],
            |row| row.get(0),
        );
        prop_assert!(
            outcome.is_ok(),
            "a search query is text someone typed, not an expression for fts5 to obey. \
             {raw:?} became {expression:?} and the parser rejected it: {:?}",
            outcome.err()
        );
    }

    #[test]
    fn ascii_punctuation_alone_searches_for_nothing(
        raw in r"[ \t!-/:-@\[-^`{-~]{0,40}"
    ) {
        prop_assert_eq!(
            codedoc_index::fts_expression(&raw),
            "",
            "with no word characters there is nothing to look for, and inventing a \
             term would return arbitrary rows for an empty question"
        );
    }

    #[test]
    fn a_word_in_any_script_stays_searchable(word in r"\p{Letter}{3,12}") {
        let expression = codedoc_index::fts_expression(&word);
        prop_assert!(
            expression.contains(&word),
            "letters outside ASCII are letters, and dropping them would make a claim \
             written in one language unsearchable in its own words: {word:?} became \
             {expression:?}"
        );
    }
}
