//! lexer_plus_text_processor.rs — временный отладочный стенд для связки `Lexer -> FrankenLab`.
//!
//! Назначение:
//! - подавать на вход текстовые chunks в формате после `TcpServer`;
//! - прогонять их через `Lexer` и `FrankenLab`;
//! - собирать `ScreenTransfer` в памяти;
//! - применять их к строковому буферу как к "виртуальному экрану".
#![cfg(test)]

#[allow(unused_imports)] use hobolib::prln;
use crate::core::core_tests::{_apply_screen_transfers, _dump_transfers, _run_pipeline};

#[test]
fn debug_spec_phrase_through_lexer_and_frankenlab() {

    // Пример из technical_specification.md, раздел 5.10.
    let chunks = &[
        "Имею ли я [5]",
        "я\nзла",
        "тые горы.*",
    ];

    let transfers = _run_pipeline(chunks);
    let final_text = _apply_screen_transfers(&transfers);

    println!("{}", final_text);

    assert_eq!(
        final_text,
        "имею я\nзлатые горы.",
        "Неожиданный поток ScreenTransfer:\n{}",
        _dump_transfers(&transfers)
    );
}   // debug_spec_phrase_through_lexer_and_frankenlab()