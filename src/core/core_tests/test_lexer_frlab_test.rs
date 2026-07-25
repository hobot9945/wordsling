//! test_lexer_frlab_test.rs — Интеграционные тесты обработки текста.
//!
//! Назначение:
//! - проверять корректность работы механизма Eager Replacement;
//! - проверять корректность отката подстановок (rollbacks) при забоях;
//! - проверять правильность сборки текста в `franken_board`.
//!
//! Проверка выполняется на виртуальном строковом буфере без вывода на реальный экран.
#![cfg(test)]

#[allow(unused_imports)] use hobolib::prln;
use crate::core::core_tests::{_process_transfers, _dump_transfers, _lexer_frlab_pipeline, mission};
use crate::core::core_tests::mission::the_vacha_river;

#[test]
fn debug_spec_phrase_through_lexer_and_frankenlab() {

    let (input, output) = the_vacha_river();

    let transfers = _lexer_frlab_pipeline(&input);
    let final_text = _process_transfers(&transfers);

    println!("{}", final_text);

    assert_eq!(
        final_text,
        output,
        "Неожиданный поток ScreenTransfer:\n{}",
        _dump_transfers(&transfers)
    );
}   // debug_spec_phrase_through_lexer_and_frankenlab()


#[test]
fn parentheses() {

    let (input, output) = mission::parentheses();

    let transfers = _lexer_frlab_pipeline(&input);
    let final_text = _process_transfers(&transfers);

    println!("{}", final_text);

    assert_eq!(
        final_text,
        output,
        "Неожиданный поток ScreenTransfer:\n{}",
        _dump_transfers(&transfers)
    );
}   // debug_spec_phrase_through_lexer_and_frankenlab()