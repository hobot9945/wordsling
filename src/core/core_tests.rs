//! core_tests.rs — хелперы для тестов.
//! 
//! Функции приватные, поскольку, дети имеют доступ к приватным полям родителя.

#![cfg(test)]
mod lexer_plus_text_processor;

use std::sync::mpsc;
#[allow(unused_imports)]
use hobolib::{prln, wrln};
use crate::core::lexer::Lexer;
use crate::core::screen_transfer::ScreenTransfer;
use crate::core::text_processor::FrankenLab;

/// Прогоняет входные chunks через связку `Lexer -> FrankenLab`
/// и возвращает поток экранных команд.
fn _run_pipeline(chunks: &[&str]) -> Vec<ScreenTransfer> {
    let (text_tx, text_rx) = mpsc::channel::<String>();
    let (lexeme_tx, lexeme_rx) = mpsc::channel();
    let (screen_tx, screen_rx) = mpsc::channel();

    let lexer = Lexer::new(text_rx, lexeme_tx);
    let franken = FrankenLab::new(lexeme_rx, screen_tx);

    for chunk in chunks {
        text_tx.send((*chunk).to_string()).unwrap();
    }   // for

    // Закрываем вход лексера. Это запускает штатное завершение всего мини-пайплайна.
    drop(text_tx);

    // Собираем весь выход FrankenLab до закрытия канала.
    let transfers: Vec<ScreenTransfer> = screen_rx.into_iter().collect();

    // Дожидаемся штатного завершения рабочих потоков.
    drop(franken);
    drop(lexer);

    transfers
}   // _run_pipeline()

/// Применяет поток `ScreenTransfer` к строковому буферу,
/// имитируя "видимый текст" на экране.
///
/// Правила:
/// - `Text(s)` дописывает текст в конец буфера;
/// - `Backspace(n)` удаляет `n` последних символов буфера.
fn _apply_screen_transfers(transfers: &[ScreenTransfer]) -> String {
    let mut buffer = String::new();

    for transfer in transfers {
        match transfer {
            ScreenTransfer::Text(text) => {
                buffer.push_str(text);
            }

            ScreenTransfer::Backspace(count) => {
                for _ in 0..*count {
                    if buffer.pop().is_none() {
                        break;
                    }   // if
                }   // for
            }
        }   // match
    }   // for

    buffer
}   // _apply_screen_transfers()

/// Формирует человекочитаемый дамп потока `ScreenTransfer`.
/// Удобно для отладочного сообщения при падении теста.
fn _dump_transfers(transfers: &[ScreenTransfer]) -> String {
    let mut out = String::new();

    for (index, transfer) in transfers.iter().enumerate() {
        match transfer {
            ScreenTransfer::Text(text) => {
                out.push_str(&format!("{:>2}: Text({:?})\n", index, text));
            }

            ScreenTransfer::Backspace(count) => {
                out.push_str(&format!("{:>2}: Backspace({})\n", index, count));
            }
        }   // match
    }   // for

    out 
}   // _dump_transfers()
