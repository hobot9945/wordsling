# Контекст проекта: Wordsling

**Суть:** Система Speech-to-Text на базе Gboard.

**Текущий фокус:** Rust-серверная часть проекта — механизм подстановок в `SurgeTable` (внутри `FrankenLab`).

## Архитектура
1. **Клиент (Android, Kotlin):** Открывает текстовое окно с клавиатурой Gboard. Вводимый/распознанный текст сериализуется и отправляется по TCP-сокету.
2. **Сервер (Rust):** Читает данные из TCP-сокета, обрабатывает их и направляет в сфокусированное текстовое поле на хосте.

## Рабочая среда
- Хостовая ОС: **Windows 10**.
- IDE: **RustRover**.
- Сервер (Rust): `C:\Users\su144\RustroverProjects\wordsling`
- Общая библиотека: `C:\Users\su144\RustroverProjects\hobolib`
- Клиент (Kotlin): `C:\Users\su144\RustroverProjects\wordsling\android`

## Важные требования по оформлению
- Код, идентификаторы и комментарии в исходниках должны быть **на английском языке**.
- Остальные требования к оформлению см. в `commenting_style.md`.
- Следи за правильностью моего английского и корректируй ошибки.
- Не исправляй код проекта самостоятельно (напрямую в файлы) без прямого указания, давай код мне, я применю.

## Текущее состояние пайплайна Wordsling server

Пайплайн полностью собран и работает end-to-end: `TcpServer -> Lexer -> FrankenLab -> ScreenWriter`
Плюс независимый `UserActivityTracker` (стаб).

Lexer, TcpServer, ScreenWriter полностью реализованы.

### Состояние FrankenLab / SurgeTable

**Реализовано и работает:**
- Две доски: `_cutting_board` (сырой текст Gboard) и `_franken_board` (экранное отражение) как `Vec<char>`.
- Стабилизационный якорь (`_stabilization_anchor`).
- Очередь экранных команд (`_screen_transfer_vec`).
- `SubstitutionMap` — словарь с prefix-aware поиском (`BTreeMap`). Нормализация запросов убрана: поиск идет по буквальному тексту.
- `SupplementaryActionMap` — реестр action/rollback пар. Action и rollback хранятся как кортеж в карте.
- FSM поиска фразового ключа с тремя состояниями:
  - `_Idle` — покой;
  - `_VacancyOpen` — вакансия открыта после `WordStart`;
  - `_CandidateGrowing` — кандидат принят в рост.
- Eager Replacement (проактивная замена):
  - при `ExactMatchWithContinuation` замена применяется немедленно во `_franken_board`;
  - сырой текст пишется только в `_cutting_board`;
  - если позже длинный ключ собирается, короткая замена честно откатывается через `_undo_exact_ready()`, затем применяется длинная;
  - если длинный ключ не собрался (`NoMatch`), короткая замена фиксируется как зубец через `_commit_exact_ready_candidate()`.
- Трёхфазная обработка лексем:
  1. Pre-phase: управление FSM (вакансии, старт кандидата);
  2. Predictive phase: оценка гипотетического кандидата (хвост cutting_board + новая лексема) ДО записи в доски;
  3. Apply phase: квалифицированное решение — куда и что писать.
- Полноценный вызов action(Before) -> replacement -> action(After) при любой подстановке, включая eager.
- Честный откат: rollback(Before) -> restore raw text -> rollback(After).
- Зубец (`_Prong`) хранит: `cb_start`, `cb_end`, `fb_start`, `fb_end`, `rollback_fn`.
- Гребёнка (`_comb`) — массив зубцов в нестабильной зоне, чистится при стабилизации.

**Ещё не реализовано (TODO):**
- Backspace (`_apply_gboard_erase`) — пока прямолинейный: режет обе доски одинаково. Не умеет:
  - откатывать зубцы при их разрушении;
  - восстанавливать surviving raw tail;
  - корректно работать с живым кандидатом.
- `WordEnd` — boundary-aware проверка кандидата (чтобы завершение слова могло убить невозможный частичный ключ).
- `_trim_comb()` — обрезка зубцов при erase (стаб).
- `_prune_if_needed()` — sliding window для ограничения роста досок (стаб).
- Реальные реализации `suppress_space_before`, `suppress_space_after` (сейчас стабы).
- Тесты для подстановок (текущий интеграционный тест `debug_spec_phrase_through_lexer_and_frankenlab` работает, но не тестирует подстановки).

## Релевантные файлы для работы
- `src/core/text_processor.rs`
- `src/core/text_processor/surgical_table.rs`
- `src/core/text_processor/substitution_map.rs`
- `src/core/text_processor/supplementary_action_map.rs`
- `src/core/core_tests/lexer_plus_text_processor.rs`

## Рекомендуемый следующий шаг
Написать юнит-тест для `SurgeTable`, который проверит eager replacement на примере ключей `"точка"` → `"."` и `"точка с запятой"` → `";"`. Это позволит убедиться, что весь механизм FSM + eager + undo + commit работает корректно, прежде чем переходить к доработке backspace.
