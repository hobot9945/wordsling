//! core_tests.rs — хелперы для тестов.
//! 
//! Функции приватные, поскольку, дети имеют доступ к приватным полям родителя.

#![cfg(test)]
mod test_lexer_frlab_test;
mod mission;
mod test_lexer_frlab_to_screen_test;

use std::sync::mpsc;
#[allow(unused_imports)]
use hobolib::{prln, wrln};
use crate::core::lexer::Lexer;
use crate::core::screen_transfer::ScreenTransfer;
use crate::core::frankenstein_laboratory::FrankenLab;

/// Прогоняет текстовые чанки через связку `Lexer` -> `FrankenLab` (мини-пайплайн).
///
/// Это интеграционный тестовый стенд. Он поднимает реальные рабочие потоки лексера
/// и процессора текста, скармливает им симуляцию TCP-пакетов (chunks) и собирает
/// итоговый поток команд для виртуального экрана.
///
/// # Аргументы
/// * `chunks` - массив строк, имитирующий фрагментированный поток от Gboard.
///
/// # Возвращает
/// Вектор команд `ScreenTransfer`, сгенерированный процессором текста.
fn _lexer_frlab_pipeline(chunks: &[&str]) -> Vec<ScreenTransfer> {
    // 1. Создаем каналы для связи этапов пайплайна.
    let (text_tx, text_rx) = mpsc::channel::<String>();
    let (lexeme_tx, lexeme_rx) = mpsc::channel();
    let (screen_tx, screen_rx) = mpsc::channel();

    // 2. Поднимаем рабочие потоки. Они начинают немедленно ждать данные.
    let lexer = Lexer::new(text_rx, lexeme_tx);
    let franken = FrankenLab::new(lexeme_rx, screen_tx);

    // 3. Скармливаем входные данные, имитируя работу TcpServer.
    for chunk in chunks {
        text_tx.send((*chunk).to_string()).unwrap();
    }   // for

    // 4. КРИТИЧЕСКИЙ ШАГ: Закрываем входной канал лексера.
    // Это запускает каскадное штатное завершение (cascading shutdown):
    // - Lexer видит обрыв text_rx, выходит из цикла и дропает свой lexeme_tx.
    // - FrankenLab видит обрыв lexeme_rx, выходит из цикла и дропает screen_tx.
    drop(text_tx);

    // 5. Собираем весь выхлоп FrankenLab.
    // collect() заблокирует текущий поток до тех пор, пока FrankenLab не закроет
    // канал screen_tx (что произойдет только после полной обработки всех данных).
    let transfers: Vec<ScreenTransfer> = screen_rx.into_iter().collect();

    // 6. Дожидаемся корректного завершения рабочих потоков (вызов join внутри drop),
    // чтобы тест не тек потоками и паники внутри потоков пробросились в тест.
    drop(franken);
    drop(lexer);

    transfers
}   // _lexer_frlab_pipeline()

use std::thread;
use std::time::Duration;
use crate::core::screen_writer::ScreenWriter;

/// Прогоняет текстовые чанки через ПОЛНЫЙ пайплайн вплоть до реального экрана.
///
/// ВНИМАНИЕ: Эта функция будет реально печатать текст и нажимать Backspace
/// в активном окне Windows! Использовать только в тестах, помеченных `#[ignore]`.
///
/// # Аргументы
/// * `chunks` - массив строк, имитирующий поток от Gboard.
fn _lexer_frlab_to_screen_pipeline(chunks: &[&str]) {
    // 1. Создаем каналы
    let (text_tx, text_rx) = mpsc::channel::<String>();
    let (lexeme_tx, lexeme_rx) = mpsc::channel();
    let (screen_tx, screen_rx) = mpsc::channel();

    // 2. Поднимаем все потоки, включая ScreenWriter
    let lexer = Lexer::new(text_rx, lexeme_tx);
    let franken = FrankenLab::new(lexeme_rx, screen_tx);
    let screen_writer = ScreenWriter::new(screen_rx);

    // 3. ПАУЗА ДЛЯ БЕЗОПАСНОСТИ!
    // Даем 3 секунды, чтобы ты успел кликнуть в окно Блокнота/Telegram.
    println!("ВНИМАНИЕ! У тебя есть 3 секунды, чтобы переключить фокус в окно для вывода текста...");
    thread::sleep(Duration::from_secs(3));
    println!("Поехали! Стреляем чанками...");

    // 4. Отстреливаем все пакеты без задержек (максимальная нагрузка)
    for chunk in chunks {
        text_tx.send((*chunk).to_string()).unwrap();
    }

    // 5. Запускаем каскадное завершение
    drop(text_tx);

    // 6. Ждем штатного завершения всех потоков.
    // ScreenWriter завершится последним, так как он должен дождаться
    // кулдаунов (_COOLDOWN_MS) и восстановить оригинальный буфер обмена.
    drop(screen_writer);
    drop(franken);
    drop(lexer);

    println!("Вывод завершен.");
}

/// Применяет поток `ScreenTransfer` к строковому буферу,
/// имитируя "видимый текст" на экране.
///
/// Правила:
/// - `Text(s)` дописывает текст в конец буфера;
/// - `Backspace(n)` удаляет `n` последних символов буфера.
fn _process_screen_transfers(transfers: &[ScreenTransfer]) -> String {
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
