//! supplementary_action_map.rs — Registry of supplementary actions for text substitutions.
//!
//! When a substitution is applied, an optional supplementary action can be triggered
//! to adjust the surrounding context (e.g., suppress whitespace before or after
//! the replacement text).
//!
//! # RESPONSIBILITY
//! - Define the `SupplementaryAction` function pointer type.
//! - Maintain a string-keyed registry of all available action implementations.
//! - Provide individual action functions that operate on the `SurgeTable`.
//!
//! # ADDING A NEW ACTION
//! 1. Implement a function with the `SupplementaryAction` signature.
//! 2. Register it in `SupplementaryActionMap::new()` under a descriptive string key.
//! 3. Use that key in `substitutions.toml` as the `modifier` value.

use std::collections::HashMap;
use crate::core::frankenstein_laboratory::surgical_table::{Prong, SurgeTable};

#[derive(Clone, Copy)]
pub(super) enum WhenApplied {
    Before,
    After,
}

/// Function pointer type for supplementary rollbacks.
///
/// # Parameters
/// - `surge_table`: mutable reference to the surgical table.
/// - `prong`: the prong being rolled back.
/// - `when_applied`: rollback phase.
pub(super) type SupplementaryRollback =
fn(surge_table: &mut SurgeTable, prong: &mut Prong, when_applied: WhenApplied);

/// Function pointer type for supplementary actions.
///
/// Actions receive mutable access to the surgical table and to the prong
/// of the substitution being applied. Adjusting the prong boundaries is the
/// primary mechanism for context manipulation: the table itself will generate
/// the corresponding screen backspaces and will restore the raw text on rollback.
///
/// # Parameters
/// - `surge_table`: mutable reference to the surgical table.
/// - `prong`: the prong of the substitution being applied.
/// - `when_applied`: indicates whether the action is called `Before` or `After`
///   the replacement text is applied.
pub(super) type SupplementaryAction =
fn(surge_table: &mut SurgeTable, prong: &mut Prong, when_applied: WhenApplied);

/// String-keyed registry of available supplementary actions.
///
/// Used during startup by `SubstitutionMap` to resolve modifier names
/// from the TOML configuration into callable function pointers.
pub(super) struct SupplementaryActionMap {
    _map: HashMap<String, (SupplementaryAction, Option<SupplementaryRollback>)>,
}   // SupplementaryActionMap

impl SupplementaryActionMap {

    /// Creates a new registry and populates it with all known actions.
    ///
    /// Every string that can appear as a `modifier` value in `substitutions.toml`
    /// must be registered here. An unregistered modifier will cause a panic
    /// at startup during `SubstitutionMap` construction.
    pub(super) fn new() -> Self {
        let mut map = HashMap::new();

        map.insert(
            "do_nothing".to_string(),
            (do_nothing as SupplementaryAction, None),
        );

        map.insert(
            "suppress_whitespace_before".to_string(),
            (suppress_whitespace_before as SupplementaryAction, None),
        );

        // Подстановка поглощает пробел справа от replacement-а.
        map.insert(
            "suppress_whitespace_after".to_string(),
            (
                suppress_whitespace_after as SupplementaryAction,
                Some(rollback_suppress_whitespace_after as SupplementaryRollback),
            ),
        );

        // Поглощает пробелы с обеих сторон replacement-а.
        map.insert(
            "suppress_whitespace_before_and_after".to_string(),
            (
                suppress_whitespace_before_and_after as SupplementaryAction,
                Some(rollback_suppress_whitespace_after as SupplementaryRollback),
            ),
        );

        // Выводить следующее слово с заглавной буквы
        map.insert(
            "capitalize_next_word".to_string(),
            (
                capitalize_next_word as SupplementaryAction,
                Some(rollback_capitalize_next_word as SupplementaryRollback),
            ),
        );

        // Сброс требования капитализации следующего слова.
        map.insert(
            "lowercase_next_word".to_string(),
            (lowercase_next_word as SupplementaryAction, None),
        );

        // Комбинированный modifier для точки:
        // - убрать пробел перед точкой;
        // - поднять флаг капитализации следующего слова.
        map.insert(
            "suppress_whitespace_before_and_capitalize_next_word".to_string(),
            (
                suppress_whitespace_before_and_capitalize_next_word as SupplementaryAction,
                Some(rollback_capitalize_next_word as SupplementaryRollback),
            ),
        );

        // Отменяет следующую подстановку, то есть следующее слово не может рассматриваться как
        // начало фразового ключа, и выводится на экран как обычный текст.
        map.insert(
            "cancel_next_replacement".to_string(),
            (
                cancel_next_replacement as SupplementaryAction,
                Some(rollback_cancel_next_replacement as SupplementaryRollback),
            ),
        );

        map.insert(
            "disable_output".to_string(),
            (disable_output as SupplementaryAction, None),
        );

        map.insert(
            "enable_output".to_string(),
            (enable_output as SupplementaryAction, None),
        );

        SupplementaryActionMap {
            _map: map,
        }
    }   // new()

    pub(super) fn get_pair(&self, key: &str)
        -> Option<(SupplementaryAction, Option<SupplementaryRollback>)>
    {
        self._map
            .get(key)
            .copied()
    }   // get_pair()

}   // impl SupplementaryActionMap

// =============================================================================
// Supplementary action implementations
// =============================================================================

/// Does nothing. Default action for substitutions that need no context adjustment.
pub(super) fn do_nothing(
    _surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    _when_applied: WhenApplied,
) {
}   // do_nothing()

/// Suppresses the whitespace character before the replacement text.
///
/// Intended for tokens that should attach to the preceding word
/// (e.g., closing parenthesis: "слово)" instead of "слово )").
///
/// # Алгоритм
/// Не трогает доски напрямую. Вместо этого сдвигает левые границы зубца
/// на один символ влево, поглощая пробел. Дальше всё делает `SurgeTable`:
/// - `_replace_candidate_tail_in_franken_board()` посчитает длину стираемого
///   хвоста от нового `_fb_start` и сгенерирует `Backspace`, накрывающий пробел;
/// - при откате зубца сырой текст будет восстановлен от нового `_cb_start`,
///   то есть пробел вернётся на экран автоматически.
///
/// # Параметры
/// - `surge_table`: хирургический стол (только для чтения досок).
/// - `prong`: зубец применяемой подстановки; его левые границы сдвигаются.
/// - `when_applied`: фаза вызова; работа выполняется только в `Before`.
pub(super) fn suppress_whitespace_before(
    surge_table: &mut SurgeTable,
    prong: &mut Prong,
    when_applied: WhenApplied,
) {
    // Сдвиг границ имеет смысл только до записи replacement-а.
    if !matches!(when_applied, WhenApplied::Before) {
        return;
    }   // if

    if surge_table._has_whitespace_before_candidate(prong) {
        prong.cb_start -= 1;
        prong.fb_start -= 1;
    }   // if
}   // suppress_whitespace_before()

/// Suppresses whitespace after the replacement text.
///
/// Intended for tokens that should attach to the following word
/// (e.g., opening parenthesis: "(слово" instead of "( слово").
///
/// # Алгоритм
/// Взводит флаг `suppress_next_whitespace` в хирургическом столе.
/// Когда придёт следующая текстовая лексема, стол проверит её:
/// если это пробел, он будет записан только в сырую доску и накрыт зубцом.
pub(super) fn suppress_whitespace_after(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.suppress_next_whitespace = true;
    }
}   // suppress_whitespace_after()

/// Откат подавления пробела после подстановки.
pub(super) fn rollback_suppress_whitespace_after(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.suppress_next_whitespace = false;
    }
}   // rollback_suppress_whitespace_after()

/// Комбинированный modifier:
/// - поглощает пробел перед replacement-ом;
/// - подавляет пробел после replacement-а.
///
/// # Алгоритм
/// Делегирует обе фазы соответствующим элементарным action-ам.
pub(super) fn suppress_whitespace_before_and_after(
    surge_table: &mut SurgeTable,
    prong: &mut Prong,
    when_applied: WhenApplied,
) {
    suppress_whitespace_before(surge_table, prong, when_applied);
    suppress_whitespace_after(surge_table, prong, when_applied);
}   // suppress_whitespace_before_and_after()

/// Взводит флаг капитализации следующего слова.
pub(super) fn capitalize_next_word(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.capitalize_next_word = true;
    }
}   // capitalize_next_word()

/// Откат капитализации при разрушении подстановки забоем.
pub(super) fn rollback_capitalize_next_word(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.capitalize_next_word = false;
    }
}   // rollback_capitalize_next_word()

/// Сбрасывает требование капитализации следующего слова.
///
/// Полезно, когда за капитализирующей подстановкой (точка, восклицательный знак)
/// идет слово, которое должно остаться строчным.
/// Работает напрямую с флагом `capitalize_next_word`, не требуя отдельного состояния.
/// Откат не реализован. Такое решение не абсолютно строго, но будет работать в большинстве случаев,
/// поскольку, вывод с маленькой буквы - это нормальное поведение.
pub(super) fn lowercase_next_word(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.capitalize_next_word = false;
    }
}   // decapitalize_next_word()

/// Комбинированный modifier:
/// - поглощает пробел перед replacement-ом;
/// - взводит капитализацию следующего слова.
///
/// # Алгоритм
/// На фазе `Before` делегирует работу `suppress_whitespace_before()`.
/// На фазе `After` делегирует работу `capitalize_next_word()`.
/// В лишних фазах вложенные функции сами ничего не делают.
pub(super) fn suppress_whitespace_before_and_capitalize_next_word(
    surge_table: &mut SurgeTable,
    prong: &mut Prong,
    when_applied: WhenApplied,
) {
    suppress_whitespace_before(surge_table, prong, when_applied.clone());
    capitalize_next_word(surge_table, prong, when_applied);
}   // suppress_whitespace_before_and_capitalize_next_word()

/// Взводит флаг отмены следующей подстановки.
///
/// Предназначен для будущей голосовой команды "буквально":
/// после её распознания ближайшее совпадение с ключевой фразой
/// должно быть пропущено, а сырой текст — выведен как есть.
pub(super) fn cancel_next_replacement(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.cancel_next_replacement = true;
    }
}   // cancel_next_replacement()

/// Откат: снимает флаг отмены следующей подстановки.
pub(super) fn rollback_cancel_next_replacement(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.cancel_next_replacement = false;
    }
}   // rollback_cancel_next_replacement()

/// Отключает вывод на экран и заказывает очистку стола для синхронизации.
///
/// При переходе в режим паузы мы сбрасываем стол, чтобы накопленный мусор
/// не вызывал рассинхрон с экраном при будущих забоях Gboard.
pub(super) fn disable_output(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.is_output_enabled = false;
        surge_table.flags.pending_clear = true;
    }
}   // disable_output()

/// Включает вывод на экран и заказывает очистку стола для старта с чистого листа.
///
/// Выход из паузы также очищает стол, чтобы сбросить любые внутренние состояния
/// и начать диктовку строго синхронно с тем, что будет на экране.
pub(super) fn enable_output(
    surge_table: &mut SurgeTable,
    _prong: &mut Prong,
    when_applied: WhenApplied,
) {
    if matches!(when_applied, WhenApplied::After) {
        surge_table.flags.is_output_enabled = true;
        surge_table.flags.pending_clear = true;
    }
}   // enable_output()
