//! test_lexer_frlab_to_screen_test.rs — ручной e2e-тест вывода на реальный экран.
//!
//! Назначение:
//! - прогнать те же входные чанки, что и в виртуальном тесте;
//! - довести пайплайн до `ScreenWriter`;
//! - вручную наблюдать артефакты вывода в реальном окне Windows.
//!
//! ВНИМАНИЕ:
//! Тест реально печатает текст и жмет Backspace в активном окне.
//! Запускать только вручную.

#![cfg(test)]

use crate::core::core_tests::_lexer_frlab_to_screen_pipeline;
use crate::core::core_tests::mission::the_vacha_river;

#[test]
#[ignore = "Пишет текст в активное окно Windows. Запускать только вручную."]
fn debug_spec_phrase_through_lexer_frlab_to_screen() {
    let (input, expected_output) = the_vacha_river();

    println!("Ожидаемый текст:\n{}\n", expected_output);
    println!("Сейчас данные будут отправлены в реальное окно вывода.");

    _lexer_frlab_to_screen_pipeline(&input);
}   // debug_spec_phrase_through_lexer_frlab_to_screen()