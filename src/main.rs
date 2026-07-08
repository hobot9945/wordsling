//! # TCP-сервер-заглушка
//!
//! Простейший блокирующий TCP-сервер для проверки сетевого контура Wordsling.
//!
//! # ОТВЕТСТВЕННОСТЬ
//! - открыть TCP-порт `0.0.0.0:51234`;
//! - принимать входящие соединения;
//! - отправлять клиенту короткий ответ-заглушку;
//! - логировать подключения и ошибки.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// Точка входа приложения.
///
/// Запускает серверную заглушку на локальном TCP-порту.
///
/// # Возвращаемые значения
/// - `std::io::Result<()>` — результат запуска и работы сервера.
///
/// # Побочные эффекты
/// - открывает TCP-порт;
/// - пишет диагностические сообщения в консоль.
fn main() -> std::io::Result<()> {
    _run_server("0.0.0.0:51234")
}   // main()

/// Запускает основной цикл приема соединений.
///
/// # Параметры
/// - `address` — локальный адрес и порт прослушивания.
///
/// # Возвращаемые значения
/// - `std::io::Result<()>` — ошибка создания слушателя или обхода входящих соединений.
///
/// # Побочные эффекты
/// - блокирует текущий поток;
/// - пишет сообщения о подключениях и ошибках в консоль.
fn _run_server(address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    println!("Wordsling TCP stub is listening on {address}");

    for stream_result in listener.incoming() {
        match stream_result {
            Ok(stream) => {
                // Соединения обрабатываются последовательно, этого достаточно для простой заглушки.
                if let Err(error) = _handle_client(stream) {
                    eprintln!("Client handling error: {error}");
                }   // if
            }
            Err(error) => {
                eprintln!("Accept error: {error}");
            }
        }   // match
    }   // for

    Ok(())
}   // _run_server()

/// Обрабатывает одно клиентское соединение.
///
/// Отправляет фиксированный текстовый ответ, затем читает данные клиента
/// и распечатывает полученные символы в консоль.
///
/// # Параметры
/// - `stream` — TCP-поток клиента.
///
/// # Возвращаемые значения
/// - `std::io::Result<()>` — результат записи ответа или чтения данных.
///
/// # Побочные эффекты
/// - записывает данные в сокет;
/// - читает данные из сокета;
/// - пишет в консоль адрес клиента и принятые символы.
fn _handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    let peer_addr = stream.peer_addr()?;
    println!("Client connected: {peer_addr}");

    // Ответ минимален и нужен только для проверки, что соединение принято сервером.
    stream.write_all(b"Wordsling TCP stub\n")?;
    stream.flush()?;

    // Читаем данные клиента и распечатываем в консоль.
    // Буфер небольшой — заглушке большого объёма не нужно.
    let mut buffer = [0u8; 1024];
    loop {

        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            // Клиент закрыл соединение.
            println!("Client disconnected: {peer_addr}");
            break;
        }   // if

        let received = &buffer[..bytes_read];
        // Распечатываем как текст, заменяя непечатаемые символы на точки.
        let text = String::from_utf8_lossy(received);
        print!("{}",text);

        // Принудительно сбрасываем буфер stdout в консоль
        std::io::stdout().flush()?;

    }   // loop

    Ok(())
}   // _handle_client()
