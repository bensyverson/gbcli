use super::*;

fn c(c: char) -> Slot {
    Some(Key::Char(c))
}

#[test]
fn a_line_lists_its_rows_cells_with_names_code_points_and_gaps() {
    let rows = read_chars("ab<heart><u+0020><>é\n<=>?\n\nz").unwrap();
    assert_eq!(
        rows[0],
        vec![
            c('a'),
            c('b'),
            Some(Key::Name("heart".into())),
            c(' '),
            None,
            c('é')
        ]
    );
    assert_eq!(rows[1], vec![c('<'), c('='), c('>'), c('?')]);
    assert_eq!(rows[2], vec![]);
    assert_eq!(rows[3], vec![c('z')]);
}

#[test]
fn a_bad_escape_or_key_is_a_problem_at_its_line_and_column() {
    let problems = read_chars("ab\nx<u+ZZ>y\u{212B}\n").unwrap_err();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!((problems[0].line, problems[0].column), (Some(2), Some(2)));
    assert_eq!((problems[1].line, problems[1].column), (Some(2), Some(9)));
    assert!(
        problems[1].message.contains("NFC"),
        "{}",
        problems[1].message
    );
}

#[test]
fn the_writer_escapes_what_would_read_back_differently() {
    let rows = vec![
        vec![
            c(' '),
            c('!'),
            c('<'),
            c('a'),
            c('<'),
            c('='),
            None,
            c('\u{301}'),
        ],
        vec![Some(Key::Name("btn_a".into())), c('<'), c('>'), None, None],
        vec![],
    ];
    let text = write_chars(&rows);
    assert_eq!(text, "<u+0020>!<u+003C>a<=<><u+0301>\n<btn_a><u+003C>>\n\n");
    let back = read_chars(&text).unwrap();
    assert_eq!(back[0], rows[0]);
    assert_eq!(back[1], rows[1][..3].to_vec());
    assert_eq!(back[2], rows[2]);
}

#[test]
fn a_list_ignores_line_breaks_gaps_and_repeats() {
    let keys = read_list("ab\nba<>\n<heart>").unwrap();
    assert_eq!(
        keys,
        vec![Key::Char('a'), Key::Char('b'), Key::Name("heart".into())]
    );
}
