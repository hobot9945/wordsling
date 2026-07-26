//! test_lexer_frlab_test.rs — интеграционные тесты обработки текста.
//!
//! Назначение:
//! - проверять корректность работы механизма Eager Replacement;
//! - проверять корректность отката подстановок (rollbacks) при забоях;
//! - проверять правильность сборки текста в `franken_board`.
//!
//! Проверка выполняется на виртуальном строковом буфере без вывода на реальный экран.
#![cfg(test)]

#[allow(unused_imports)] use hobolib::prln;
use crate::core::core_tests::{_process_screen_transfers, _dump_transfers, _lexer_frlab_pipeline, mission};
use crate::core::core_tests::mission::the_vacha_river;

#[test]
fn debug_spec_phrase_through_lexer_and_frankenlab() {

    let (input, output) = the_vacha_river();

    let transfers = _lexer_frlab_pipeline(&input);
    let final_text = _process_screen_transfers(&transfers);

    println!("{}", final_text);

    assert_eq!(
        final_text,
        output,
        "Неожиданный поток ScreenTransfer:\n{}",
        _dump_transfers(&transfers)
    );
}   // debug_spec_phrase_through_lexer_and_frankenlab()


#[test]
fn parentheses_and_capitalization() {

    let (input, output) = mission::parentheses_capitalization();

    let screen_transfers = _lexer_frlab_pipeline(&input);
    let final_text = _process_screen_transfers(&screen_transfers);

    println!("{}", final_text);

    assert_eq!(
        final_text,
        output,
        "Неожиданный поток ScreenTransfer:\n{}",
        _dump_transfers(&screen_transfers)
    );
}   // debug_spec_phrase_through_lexer_and_frankenlab()

#[test]
fn dot_capitalization() {

    let (input, output) = mission::dot_capitalization();

    let screen_transfers = _lexer_frlab_pipeline(&input);
    let final_text = _process_screen_transfers(&screen_transfers);

    println!("{}", final_text);

    assert_eq!(
        final_text,
        output,
        "Неожиданный поток ScreenTransfer:\n{}",
        _dump_transfers(&screen_transfers)
    );
}   // dot_capitalization()