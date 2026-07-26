//! surgical_table.rs — The surgical table for text processing.
//!
//! Manages the raw input from Gboard (`cutting_board`) and the processed
//! output mirroring the Windows screen (`franken_board`).
//!
//! Текущая идея алгоритма:
//!
//! 1. Сначала автомат поиска решает, может ли лексема открыть кандидата.
//! 2. Потом оценивается гипотетический кандидат:
//!    уже накопленный хвост cutting_board + новая значимая лексема.
//! 3. После этого принимается решение:
//!    - писать сырой текст в обе доски;
//!    - или писать сырой текст только в cutting_board,
//!      а во franken_board выполнить eager replacement.

use hobolib::prln;
use crate::core::lexeme_transfer::LexemeTransfer;
use crate::core::screen_transfer::ScreenTransfer;
use crate::core::frankenstein_laboratory::substitution_map::{
    SubstitutionMap,
    SubstitutionSearchResult,
};
use crate::core::frankenstein_laboratory::supplementary_action_map::{
    SupplementaryAction,
    SupplementaryRollback,
    WhenApplied,
};

/// Maximum number of characters retained in the boards.
const _MAX_BOARD_CAPACITY: usize = 2000;

/// Represents mapping between a segment in the cutting board
/// and the corresponding segment in the franken board.
#[derive(Default, Clone)]
pub(super) struct Prong {
    pub(super) cb_start: usize,
    _cb_end: usize,
    pub(super) fb_start: usize,
    _fb_end: usize,
    _rollback_fn: Option<SupplementaryRollback>,
}   // _Prong

/// Разные служебные флаги проекта.
pub(super) struct Flag {
    pub(super) suppress_next_whitespace: bool,
    pub(super) capitalize_next_word: bool,
    pub(super) cancel_next_replacement: bool,
}

impl Default for Flag {
    fn default() -> Self {
        Self {
            suppress_next_whitespace: true,     // пробел перед первым словом не нужен
            capitalize_next_word: true,         // по умолчанию первое слово с заглавной
            cancel_next_replacement: false,
        }
    }
}

impl Flag {
    fn new() -> Self {
        Self::default()
    }

    fn _clear(&mut self) {
        self.suppress_next_whitespace = true;   // история очищена, значит разделение пробелом не нужно
        self.capitalize_next_word = false;
        self.cancel_next_replacement = false;
    }
}   // impl Flag

// =============================================================================
// Search FSM
// =============================================================================

/// Семантический статус уже открытого кандидата.
///
/// `_Partial`
///     Кандидат растет, но точного совпадения пока не было.
///
/// `_ExactReady`
///     Точное совпадение уже было найдено и полностью применено:
///     `action(Before) -> replacement -> action(After)`.
///
///     Но оно еще не зафиксировано окончательно в гребенке,
///     потому что словарь допускает более длинное продолжение.
///
/// Если позже короткий exact будет перебит более длинным ключом,
/// эта ранняя подстановка должна быть честно откачена через:
/// `rollback(Before) -> restore raw text -> rollback(After)`.
#[derive(Clone)]
enum _CandidateStatus {
    _Partial,
    _ExactReady
}   // _CandidateStatus

/// Живой кандидат, который сейчас растет.
#[derive(Clone)]
struct _Candidate {
    /// `_prong._cb_start` / `_prong._fb_start`
    ///     начало всего растущего кандидата.
    ///
    /// `_prong._cb_end` / `_prong._fb_end`
    ///     границы последнего "готового" точного совпадения внутри кандидата,
    ///     если кандидат когда-либо входил в `_ExactReady`.
    _prong: Prong,

    /// Текущий статус роста кандидата.
    _status: _CandidateStatus,
}   // _Candidate

/// Автомат поиска фразового ключа.
///
/// `_Idle`
///     Сейчас никакой ключ не отслеживается.
///
/// `_VacancyOpen`
///     Пришла лексема `WordStart`.
///     Следующий первый `WordPart` имеет право стать кандидатом на начало
///     заменяемого текста.
///
///     Пунктуация в это состояние не нуждается:
///     она может открыть кандидата напрямую из `_Idle`.
///
/// `_CandidateGrowing`
///     Кандидат уже принят в рост.
///     На каждой значимой лексеме его гипотетическая строка перепроверяется
///     в словаре.
enum _CandidatePosition {
    _None,
    _VacancyOpen,
    _CandidateGrowing(_Candidate),
}   // _CandidatePosition

impl Default for _CandidatePosition {
    fn default() -> Self {
        Self::_None
    }
}   // impl Default for _CandidatePosition

/// Решение словарного слоя, отвязанное от заимствований из `SubstitutionMap`.
enum _SearchDecision {
    _NoMatch,
    _Partial,
    _ExactReady {
        _replacement_text: String,
        _action: SupplementaryAction,
        _rollback_fn: Option<SupplementaryRollback>,
    },
    _ApplyNow {
        _replacement_text: String,
        _action: SupplementaryAction,
        _rollback_fn: Option<SupplementaryRollback>,
    },
}   // _SearchDecision

impl _SearchDecision {

    /// Строит безопасное решение поиска без удержания borrow на словаре.
    fn _make_decision(subst_map: &SubstitutionMap, query: &str, is_final: bool) -> Self {
        let search_result = if is_final {
            subst_map.final_search(query)
        } else {
            subst_map.search(query)
        };

        match search_result {
            SubstitutionSearchResult::NoMatch => {
                _SearchDecision::_NoMatch
            }

            SubstitutionSearchResult::PartialMatch => {
                _SearchDecision::_Partial
            }

            SubstitutionSearchResult::ExactMatch(element) => {
                _SearchDecision::_ApplyNow {
                    _replacement_text: element.replacement_text().to_string(),
                    _action: element.action(),
                    _rollback_fn: element.rollback(),
                }
            }

            SubstitutionSearchResult::ExactMatchWithContinuation(element) => {
                _SearchDecision::_ExactReady {
                    _replacement_text: element.replacement_text().to_string(),
                    _action: element.action(),
                    _rollback_fn: element.rollback(),
                }
            }
        }
    }   // _make_decision()

}   // impl _SearchDecision

// =============================================================================
// Main table
// =============================================================================

pub(super) struct SurgeTable {
    /// Raw text received from Gboard.
    _cutting_board: Vec<char>,

    /// Processed text mirroring the screen content.
    _franken_board: Vec<char>,

    // Точка, до которой хранится история, а после которой доска франкенштейна превращается в
    // отображение разделочной доски.
    _fb_rubicon: usize,

    /// Активные зубцы в нестабильной зоне.
    /// Здесь лежат только уже состоявшиеся подстановки.
    _comb: Vec<Prong>,

    /// Текущее состояние автомата поиска фразового ключа.
    _candidate_position: _CandidatePosition,

    pub(super) flags: Flag,

    /// Карта подстановок.
    _subst_map: SubstitutionMap,

    /// Internal queue of screen commands awaiting dispatch.
    _screen_transfer_vec: Vec<ScreenTransfer>,
}   // SurgeTable

impl SurgeTable {

    /// Constructor.
    pub(super) fn new() -> Self {
        SurgeTable {
            _cutting_board: Vec::new(),
            _franken_board: Vec::new(),
            _fb_rubicon: 0,
            _comb: Vec::new(),
            _candidate_position: _CandidatePosition::_None,
            flags: Flag::new(),
            _subst_map: SubstitutionMap::new(),
            _screen_transfer_vec: Vec::new(),
        }
    }   // new()

}   // impl SurgeTable

impl SurgeTable {

    /// Processes a single incoming lexeme.
    ///
    /// Алгоритм разбит на 3 фазы:
    ///
    /// 1. pre-phase
    ///    Обновляем FSM поиска ДО записи новой лексемы в доски.
    ///
    /// 2. predictive phase
    ///    Если лексема значимая, ищем гипотетического кандидата в карте замен:
    ///    текущий хвост кандидата + новая лексема.
    ///
    /// 3. apply phase
    ///    После оценки принимаем квалифицированное решение, что писать:
    ///    - сырой текст в обе доски,
    ///    - или сырой текст только в cutting_board, а во franken_board
    ///      уже замену.
    pub(super) fn process_lexeme(&mut self, lexeme: &LexemeTransfer) {

        self._preprocess_employing_candidate(lexeme);

        if let Some(text_lexeme) = Self::_extract_significant_text(lexeme) {
            self._process_text_lexeme(&text_lexeme);
        } else {
            self._process_service_lexeme(lexeme);
        }
    }   // process_lexeme()

    /// Extracts all accumulated screen transfers and clears the internal queue.
    pub(super) fn pop_screen_transfers(&mut self) -> Vec<ScreenTransfer> {
        std::mem::take(&mut self._screen_transfer_vec)
    }   // pop_screen_transfers()

    /// Извлекает значимый текст лексемы.
    ///
    /// Для:
    /// - `WordPart`
    /// - `Whitespace`
    /// - `Punctuation`
    ///
    /// Возвращает строку, которую эта лексема добавляет в поток.
    fn _extract_significant_text(lexeme: &LexemeTransfer) -> Option<String> {
        match lexeme {
            LexemeTransfer::WordPart(text) => Some(text.clone()),
            LexemeTransfer::Whitespace(c) | LexemeTransfer::Punctuation(c) => {
                Some(c.to_string())
            }

            LexemeTransfer::WordStart
            | LexemeTransfer::WordEnd
            | LexemeTransfer::EraseStart
            | LexemeTransfer::BackspaceCount(_)
            | LexemeTransfer::EraseEnd
            | LexemeTransfer::Stabilization
            | LexemeTransfer::UserActivityDetected => {
                None
            }
        }
    }   // _extract_significant_text()
}   // impl SurgeTable

// =============================================================================
// Phase 1: preprocess FSM
// =============================================================================

impl SurgeTable {

    /// Подготовительный этап перед обработкой лексемы.
    ///
    /// Здесь мы решаем только одно:
    /// может ли лексема открыть нового кандидата.
    fn _preprocess_employing_candidate(&mut self, lexeme: &LexemeTransfer) {

        // Обработка флага отмены следующей подстановки (команда "буквально").
        // Если флаг поднят, мы ждём лексему, способную открыть кандидата.
        // Когда она приходит — гасим флаг и оставляем статус _None,
        // чтобы текст ушёл в доски как сырой, без подстановки.
        if self.flags.cancel_next_replacement {
            if matches!(lexeme, LexemeTransfer::WordStart | LexemeTransfer::Punctuation(_)) {
                self.flags.cancel_next_replacement = false;
                self._candidate_position = _CandidatePosition::_None;
                return;
            }
        }

        match lexeme {

            LexemeTransfer::WordStart => {
                // Открываем вакансию:
                // следующий первый WordPart может стать началом кандидата.
                if matches!(self._candidate_position, _CandidatePosition::_None) {
                    self._candidate_position = _CandidatePosition::_VacancyOpen;
                }
            }

            LexemeTransfer::WordPart(_) => {
                // Первый WordPart после WordStart создает кандидата.
                // Сам по себе WordPart не должен стартовать кандидата из _Idle,
                // иначе можно случайно начать нового кандидата с середины уже растущего слова,
                // разбитого транспортом на несколько кусков.
                if matches!(self._candidate_position, _CandidatePosition::_VacancyOpen) {
                    self._candidate_position = self._new_candidate();
                }
            }

            LexemeTransfer::Punctuation(_) => {
                // Пунктуация может сама по себе быть началом ключа.
                // Поэтому она имеет право открыть кандидата не только из _VacancyOpen,
                // но и напрямую из _Idle.
                //
                // Если кандидат уже растет, ничего не делаем: новая пунктуация будет
                // обработана как продолжение текущего кандидата на следующей стадии.
                if matches!(
                    self._candidate_position,
                    _CandidatePosition::_None | _CandidatePosition::_VacancyOpen
                ) {
                    self._candidate_position = self._new_candidate();
                }
            }

            LexemeTransfer::Whitespace(_) => {
                // Пробел сам по себе нового кандидата не открывает.
                // Если "вакансия" висела, но слово так и не началось — гасим ее.
                if matches!(self._candidate_position, _CandidatePosition::_VacancyOpen) {
                    self._candidate_position = _CandidatePosition::_None;
                }
            }

            LexemeTransfer::BackspaceCount(_)
            | LexemeTransfer::Stabilization
            | LexemeTransfer::WordEnd
            | LexemeTransfer::UserActivityDetected
            | LexemeTransfer::EraseStart
            | LexemeTransfer::EraseEnd => {
                // No-op
            }

        }   // match
    }   // _preprocess_employing_candidate()

    /// Создает нового растущего кандидата, начиная с текущего хвоста обеих досок.
    fn _new_candidate(&mut self) -> _CandidatePosition {
        _CandidatePosition::_CandidateGrowing(_Candidate {
            _prong: Prong {
                cb_start: self._cutting_board.len(),
                _cb_end: 0,
                fb_start: self._franken_board.len(),
                _fb_end: 0,
                _rollback_fn: None,
            },
            _status: _CandidateStatus::_Partial,
        })
    }   // _new_candidate

}   // impl SurgeTable

// =============================================================================
// Phase 2 + 3: predictive evaluation and apply
// =============================================================================

impl SurgeTable {

    /// Обрабатывает значимую текстовую лексему по новому алгоритму.
    ///
    /// Ключевая разница с прежней схемой:
    /// мы НЕ пишем лексему заранее в обе доски.
    /// Сначала оцениваем гипотетический кандидат,
    /// и только потом решаем, как именно записывать.
    fn _process_text_lexeme(&mut self, lexeme_text: &str) {

        // Перехватчик подавления пробела
        if self.flags.suppress_next_whitespace {
            self.flags.suppress_next_whitespace = false;

            if !lexeme_text.is_empty() && lexeme_text.chars().all(|c| c.is_whitespace()) {
                // Пишем пробел только в сырую доску
                self._write_raw_text_to_cutting_board(lexeme_text);

                // Прячем этот пробел в зубец, который инициировал подавление
                if let _CandidatePosition::_CandidateGrowing(ref mut candidate) = self._candidate_position {
                    // Нужный зубец находится в кандидате
                    candidate._prong._cb_end = self._cutting_board.len();
                } else if let Some(last_prong) = self._comb.last_mut() {
                    // Кандидата нет, значит нужный зубец в гребне.
                    last_prong._cb_end = self._cutting_board.len();
                }

                // Прерываем обработку: пробел "проглочен" и не вызовет сдвигов FSM
                return;
            }
        }

        // Вынимаем FSM из self, чтобы можно было свободно мутировать self дальше. take() выполняет
        // self._candidate_position = _CandidatePosition::default().
        let candidate_position = std::mem::take(&mut self._candidate_position);

        // Принять текст новой лексемы.
        match candidate_position {

            // Тривиальная часть. Кандидат не был образован, новый текст просто пишем в обе доски.
            _CandidatePosition::_None | _CandidatePosition::_VacancyOpen => {
                // _None - кандидата не было, то есть нет истории. Пишем новый текст в обе доски,
                // генерируем текст для передачи на экран.
                // _VacancyOpen - после preprocess для текстовой лексемы сюда попасть не должны.
                // То есть, готовы были принять кандидата, и пришел текст, чтобы им стать, но,
                // почему-то не стал. На всякий случай, обеспечим безопасное поведение.
                self._write_raw_text_to_both_boards(lexeme_text);
                self._candidate_position = _CandidatePosition::_None;
            }

            // Содержательная часть. Кандидат образован, пусть, даже пустой.
            _CandidatePosition::_CandidateGrowing(candidate) => {
                // Плюсуем новую лексему к тексту кандидата.
                let candidate_new_text =
                    self._build_candidate_string_new_text_included(&candidate._prong, lexeme_text);

                // Сверяем новый текст с картой подстановок, генерируем решение.
                let decision =
                    _SearchDecision::_make_decision(&self._subst_map, &candidate_new_text, false);

                // Применяем решение.
                self._candidate_position = self._apply_decision(candidate, lexeme_text, decision);
            }
        }   // match
    }   // _process_text_lexeme()

    /// Применяет решение, принятое по гипотетическому кандидату. Когда функция вызывается, кандидат
    /// всегда есть (_CandidateGrowing), его статус либо _Partial, либо _ExactReady.
    fn _apply_decision(
        &mut self,
        mut candidate: _Candidate,
        new_lexeme_text: &str,
        decision: _SearchDecision)
        -> _CandidatePosition
    {
        // Кандидат есть (CandidatePosition::_CandidateGrowing), его статус либо _Partial, даже если
        // он пустой (только образован), либо _ExactReady.
        match decision {

            // Кандидата либо принимаем в партию, либо расстреливают.
            _SearchDecision::_NoMatch => {
                // Новая лексема либо дисквалифицирует _Partial кандидата, либо завершает _ExactReady.
                //
                // Текущее поведение:
                // - пишем новую лексему сырьем в обе доски;
                // - если ранее уже был eager exact, принимаем кандидата в партию имени Франкенштейна,
                // то есть фиксируем его зубец в гребне;
                self._write_raw_text_to_both_boards(new_lexeme_text);

                if matches!(candidate._status, _CandidateStatus::_ExactReady) {
                    // Кандидат становится подстановкой, освобождая место кандидата.
                    self._commit_exact_ready_candidate(candidate);
                    _CandidatePosition::_None
                } else {
                    // Кандидат не оправдал доверия и приговорен к расстрелу.
                    _CandidatePosition::_None
                }
            }

            // Кандидата либо оставляем пока в живых, либо, если он уже принят в партию в качестве
            // исполняющего обязанности, намекают на возможность повышения (совпал с коротким вариантом
            // фразового ключа, выполнил подстановку, но еще может дорасти до более длинного варианта).
            _SearchDecision::_Partial => {
                // Кандидат еще может расти. Новая лексема пока не вызывает замену, поэтому просто пишем
                // ее сырьем в обе доски.
                //
                // Если у кандидата уже был статус ExactReady, подстановка уже выполнена, но его зубец
                // еще не переехал в гребень. Статус сохраняется. Кандидат еще может утвердиться как
                // подстановка, если не случится более длинное совпадение. Оставляем его в ожидании.
                self._write_raw_text_to_both_boards(new_lexeme_text);
                _CandidatePosition::_CandidateGrowing(candidate)
            }

            // Кандидата либо принимаем в партию в качестве исполняющего обязанности, либо продвигают
            // выше. Но, оставляем исполняющим, намекая на возможность дальнейшего роста.
            _SearchDecision::_ExactReady {
                _replacement_text,
                _action,
                _rollback_fn,
            } => {
                // Новый сырой текст принимается только в cutting_board.
                self._write_raw_text_to_cutting_board(new_lexeme_text);

                // Поверх актуального кандидата применяем новый eager exact. Гребень не затрагивается.
                self._apply_substitution_for_candidate(
                    &mut candidate,
                    &_replacement_text,
                    _action,
                    _rollback_fn,
                );

                // Установить новое состояние.
                candidate._status = _CandidateStatus::_ExactReady;
                _CandidatePosition::_CandidateGrowing(candidate)
            }

            _SearchDecision::_ApplyNow {
                _replacement_text,
                _action,
                _rollback_fn,
            } => {
                // Новый сырой текст принимается только в cutting_board.
                self._write_raw_text_to_cutting_board(new_lexeme_text);

                // Выполняем окончательную exact-подстановку.
                self._apply_substitution_for_candidate(
                    &mut candidate,
                    &_replacement_text,
                    _action,
                    _rollback_fn,
                );

                // Фиксируем зубец в гребне.
                self._comb.push(candidate._prong);

                // Освобождаем место для нового кандидата.
                _CandidatePosition::_None
            }
        }   // match
    }   // _apply_text_decision()
}   // impl SurgeTable

// =============================================================================
// Service lexemes
// =============================================================================

impl SurgeTable {

    /// Обрабатывает нетекстовые лексемы.
    fn _process_service_lexeme(&mut self, lexeme: &LexemeTransfer) {
        match lexeme {

            LexemeTransfer::Stabilization => {
                self._finalize_candidate_on_stabilization();
                self._mark_gboard_stabilization();
                self._candidate_position = _CandidatePosition::_None;
            }

            LexemeTransfer::BackspaceCount(n) => {
                self._apply_gboard_erase(*n as usize);
            }

            LexemeTransfer::UserActivityDetected => {
                // Активность мыши или клавиатуры - это вынужденная стабилизация, то есть финализация
                // кандидата, а потом очистка всего.
                self._finalize_candidate_on_stabilization();
                self._clear_all();
                self._candidate_position = _CandidatePosition::_None;
            }

            LexemeTransfer::WordEnd => {
                // Закрываем "вакансию", если слово так и не началось.
                if matches!(self._candidate_position, _CandidatePosition::_VacancyOpen) {
                    self._candidate_position = _CandidatePosition::_None;
                }
            }

            LexemeTransfer::WordStart
            | LexemeTransfer::EraseStart
            | LexemeTransfer::EraseEnd => {
                // Эти лексемы уже отработали на pre-phase
                // или не требуют отдельной сервисной обработки.
            }

            LexemeTransfer::WordPart(_)
            | LexemeTransfer::Whitespace(_)
            | LexemeTransfer::Punctuation(_) => {
                // Сюда попадать не должны:
                // они обрабатываются через _process_text_lexeme().
            }

        }   // match
    }   // _process_service_lexeme()

    /// Финализирует живого кандидата при стабилизации потока.
    ///
    /// Здесь новая лексема уже не прибавляется.
    /// Мы оцениваем накопленный кандидатом текст "как есть".
    fn _finalize_candidate_on_stabilization(&mut self) {

        // Забираем владение енумом позиции кандидата из структуры разделочного стола. Его
        // уже не вернем. Там останется дефолт, то есть _CandidatePosition::_None.
        let candidate_position = std::mem::replace(&mut self._candidate_position,
                                                   _CandidatePosition::_None);

        // Нас интересует только живой кандидат. Если кандидата нет, нечего подставлять.
        let _CandidatePosition::_CandidateGrowing(candidate) = candidate_position else {
            return;
        };

        // Выделить накопленный кандидатом текст и искать его в карте замен.
        let candidate_string = self._get_candidate_string(&candidate._prong);
        let decision = _SearchDecision::_make_decision(&self._subst_map, &candidate_string, true);

        match decision {

            _SearchDecision::_ApplyNow {
                _replacement_text,
                _action,
                _rollback_fn,
            } => {
                // Выполнить подстановку.
                let mut candidate = candidate;
                self._apply_substitution_for_candidate(
                    &mut candidate,
                    &_replacement_text,
                    _action,
                    _rollback_fn,
                );

                // Финализировать подстановку передачей зубца в гребень.
                self._comb.push(candidate._prong);
            }

            _SearchDecision::_ExactReady { .. } => {
                // Для final_search() такого быть не должно.
                unreachable!("final_search() must not return _ExactReady");
            }

            _SearchDecision::_Partial | _SearchDecision::_NoMatch => {
                // Полный текущий кандидат не собрался в окончательный exact.
                // Если внутри него ранее уже было краткое ExactReady,
                // считаем его состоявшимся.
                if matches!(candidate._status, _CandidateStatus::_ExactReady) {
                    self._commit_exact_ready_candidate(candidate);
                }
            }

        }   // match
    }   // _finalize_candidate_on_stabilization()

}   // impl SurgeTable

// =============================================================================
// Candidate helpers
// =============================================================================

impl SurgeTable {

    /// Извлекает текущую строку кандидата из cutting_board.
    fn _get_candidate_string(&self, prong: &Prong) -> String {
        self._cutting_board[prong.cb_start..]
            .iter()
            .collect::<String>()
    }   // _get_candidate_string()

    /// Строит гипотетическую строку кандидата:
    /// текущий хвост cutting_board + новая значимая лексема.
    fn _build_candidate_string_new_text_included(&self, prong: &Prong, text: &str) -> String {
        let mut out = self._get_candidate_string(prong);
        out.push_str(text);
        out
    }   // _build_candidate_string_new_text_included()

    /// Фиксирует ранее найденный eager exact как полноценный зубец.
    fn _commit_exact_ready_candidate(&mut self, candidate: _Candidate) {
        if matches!(candidate._status, _CandidateStatus::_ExactReady) {
            self._comb.push(candidate._prong);
        }
    }   // _commit_exact_ready_candidate()

    /// Выполняет exact-подстановку над текущим кандидатом и обновляет его зубец. Если кандидат
    /// в состоянии _ExactReady, а значит, отвечает за последнюю подстановку, откатываем его и
    /// подстановку перед накатом текущей.
    ///
    /// Предполагается, что:
    /// - новый сырой текст текущей лексемы (если он есть) уже принят в cutting_board.
    fn _apply_substitution_for_candidate(
        &mut self,
        candidate: &mut _Candidate,
        replacement_text: &str,
        action: SupplementaryAction,
        rollback_fn: Option<SupplementaryRollback>,
    ) {
        // Если короткий eager exact уже висит на экране,
        // сначала честно откатываем его обратно к сырому виду кандидата.
        if matches!(candidate._status, _CandidateStatus::_ExactReady) {
            self._undo_exact_ready(candidate);
        }

        action(self, &mut candidate._prong, WhenApplied::Before);

        self._replace_candidate_tail_in_franken_board(
            candidate._prong.fb_start,
            replacement_text,
        );

        action(self, &mut candidate._prong, WhenApplied::After);

        candidate._prong._cb_end = self._cutting_board.len();
        candidate._prong._fb_end = self._franken_board.len();
        candidate._prong._rollback_fn = rollback_fn;

    }   // _apply_substitution_for_candidate()

    /// Откатывает состоявшуюся раннюю подстановку по всем правилам:
    /// вызов rollback(Before) -> восстановление сырого текста -> вызов rollback(After).
    /// Гребень не затрагивается, зубец еще во владении кандидата.
    fn _undo_exact_ready(&mut self, candidate: &mut _Candidate) {
        if matches!(candidate._status, _CandidateStatus::_ExactReady) {

            // Выполнить дополнительные откатные действия до стирания подстановки.
            if let Some(rb) = candidate._prong._rollback_fn {
                rb(self, &mut candidate._prong, WhenApplied::Before);
            }

            // Стереть подстановку.
            let raw_tail_len = self._franken_board.len() - candidate._prong.fb_start;
            if raw_tail_len > 0 {
                self._franken_board.truncate(candidate._prong.fb_start);
                self._screen_transfer_vec.push(ScreenTransfer::Backspace(raw_tail_len));
            }

            // Заменить на сырой текст из разделочной доски.
            let original_raw_text: String = self._cutting_board[candidate._prong.cb_start..].iter().collect();
            if !original_raw_text.is_empty() {
                self._franken_board.extend(original_raw_text.chars());
                self._screen_transfer_vec.push(ScreenTransfer::Text(original_raw_text));
            }

            // Выполнить дополнительные откатные действия после отката подстановки.
            if let Some(rb) = candidate._prong._rollback_fn {
                rb(self, &mut candidate._prong, WhenApplied::After);
            }

            // Привести зубец в исходное состояние.
            candidate._prong._cb_end = 0;
            candidate._prong._fb_end = 0;
            candidate._prong._rollback_fn = None;

        }
    }   // _undo_exact_ready()
}   // impl SurgeTable

// =============================================================================
// Low-level board operations
// =============================================================================

impl SurgeTable {

    /// Пишет сырой текст сразу в обе доски.
    fn _write_raw_text_to_both_boards(&mut self, text: &str) {
        self._write_raw_text_to_cutting_board(text);
        self._write_raw_text_to_franken_board(text);
    }   // _write_raw_text_to_both_boards()

    /// Пишет сырой текст в cutting_board.
    fn _write_raw_text_to_cutting_board(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let chars: Vec<char> = text.chars().collect();
        self._cutting_board.extend_from_slice(&chars);
    }   // _write_raw_text_to_cutting_board()

    /// Пишет сырой текст во franken_board и генерирует экранную передачу.
    fn _write_raw_text_to_franken_board(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // Применяем капитализацию только для franken_board и экрана
        let processed_text = self._capitalize_if_needed(text);

        let chars: Vec<char> = processed_text.chars().collect();
        self._franken_board.extend_from_slice(&chars);
        self._screen_transfer_vec
            .push(ScreenTransfer::Text(processed_text));
        self._prune_if_needed();
    }   // _write_raw_text_to_franken_board()

    /// Заменяет весь текущий franken-tail кандидата replacement-ом.
    ///
    /// Важный нюанс:
    /// новая лексема здесь еще НЕ была записана во franken_board.
    /// Поэтому мы стираем только уже существующий хвост кандидата.
    fn _replace_candidate_tail_in_franken_board(&mut self, fb_start: usize,
                                                replacement_text: &str)
    {
        // Рассчитать длину стираемого хвоста.
        let raw_tail_len = self._franken_board.len() - fb_start;

        // Стереть хвост.
        if raw_tail_len > 0 {
            self._franken_board.truncate(fb_start);
            self._screen_transfer_vec
                .push(ScreenTransfer::Backspace(raw_tail_len));
        }

        // Дописать замену.
        if !replacement_text.is_empty() {
            self._franken_board.extend(replacement_text.chars());
            self._screen_transfer_vec
                .push(ScreenTransfer::Text(replacement_text.to_string()));
        }
    }   // _replace_candidate_tail_in_franken_board()

}   // impl SurgeTable

// =============================================================================
// Misc state management
// =============================================================================

impl SurgeTable {

    /// Применяет забои, присланные Gboard.
    ///
    /// Идея алгоритма:
    /// 1. Сначала решаем судьбу живого кандидата.
    ///    - Если забои не достигают кандидата, оставляем его в покое.
    ///    - Если забои разрушают `_Partial` кандидата, просто убиваем его:
    ///      подстановки там еще не было.
    ///    - Если забои достигают состоявшейся eager-подстановки живого кандидата
    ///      (`_ExactReady`), переносим его зубец в гребень, но пока не откатываем.
    ///      Дальше цикл сам обработает его как обычный последний зубец.
    ///
    /// 2. Потом идем справа налево, пока не исчерпаем заказанные забои:
    ///    - либо съедаем свободный хвост, одинаковый в обеих досках;
    ///    - либо разрушаем последний зубец:
    ///         rollback(Before) -> стереть replacement ->
    ///         восстановить surviving raw tail -> rollback(After).
    ///
    /// Важное следствие:
    /// забой свободного хвоста не обязан убивать кандидата.
    /// Это нужно для сценария:
    ///     "точка с за" + "[2]запятой"
    /// где кандидат должен пережить стирание хвоста "за" и дорасти до
    /// "точка с запятой".
    fn _apply_gboard_erase(&mut self, mut requested_erase: usize) {
        if requested_erase == 0 || self._cutting_board.is_empty() {
            return;
        }

        // ---------------------------------------------------------------------
        // Шаг 1. Судьба живого кандидата.
        // ---------------------------------------------------------------------

        // Позиция в разделочной доске, до куда достигают забои.
        let target_cb_len = self._cutting_board.len().saturating_sub(requested_erase);

        enum _CandidateDestiny {
            /// Кандидата не трогаем: забои его не достигают.
            _Keep,

            /// Кандидата расстрелять: забои его накрыли, а подстановка для него еще не нашлась.
            _Drop,

            /// Кандидат в `_ExactReady` уже несет подстановку.
            /// Если забои достигают его exact-части, переносим его зубец в гребень,
            /// чтобы дальше общий цикл обработал его как обычный последний зубец.
            _CommitExactReadyToComb,
        }   // _CandidateEraseReaction

        // Определить candidate_destiny.
        let candidate_destiny = match &self._candidate_position {
            _CandidatePosition::_CandidateGrowing(candidate) => {
                match candidate._status {
                    _CandidateStatus::_ExactReady => {
                        // exact-ready кандидат:  уже есть подстановка, но она может измениться.
                        if target_cb_len < candidate._prong._cb_end {
                            // Кандидат затронут забоями, подлежит фиксации в гребне
                            _CandidateDestiny::_CommitExactReadyToComb
                        } else {
                            // Забои не дошли до кандидата, можно его не трогать
                            _CandidateDestiny::_Keep
                        }
                    }

                    _CandidateStatus::_Partial => {
                        // partial-кандидат: подстановки еще нет, хотя с началом фразового ключа он совпал;
                        // _prong._cb_start - начало возможной подстановки, _prong._cb_end = 0 (не определен).
                        if target_cb_len < candidate._prong.cb_start {
                            // Покрыт забоями: приговорен к расстрелу.
                            _CandidateDestiny::_Drop
                        } else {
                            // Забои не дошли до него: пусть живет.
                            _CandidateDestiny::_Keep
                        }
                    }
                }   // match
            }

            // "Вакансия": кандидата еще нет. Забой говорит, что и не будет.
            _CandidatePosition::_VacancyOpen => {
                _CandidateDestiny::_Drop    // приведет к закрытию вакансии
            }

            // Кандидата нет. Судьба не имеет значения.
            _CandidatePosition::_None => {
                _CandidateDestiny::_Keep
            }
        };   // match

        // Исполнить candidate_destiny.
        match candidate_destiny {
            _CandidateDestiny::_Keep => {
                // Оставить кандидата в покое.
            }

            _CandidateDestiny::_Drop => {
                // Расстрелять беднягу.
                self._candidate_position = _CandidatePosition::_None;
            }

            _CandidateDestiny::_CommitExactReadyToComb => {
                // Кандидат есть, он выполнил подстановку. Теперь он подлежит фиксации в гребне.

                // Забрать владение позицией. Место для нового кандидата освобождается.
                let candidate_position = std::mem::replace(
                    &mut self._candidate_position,
                    _CandidatePosition::_None,
                );

                // Забрать владение кандидатом.
                let _CandidatePosition::_CandidateGrowing(candidate) = candidate_position else {
                    unreachable!("candidate erase reaction diverged from candidate state");
                };

                debug_assert!(matches!(candidate._status, _CandidateStatus::_ExactReady));

                // ВАЖНО: здесь мы НЕ откатываем подстановку. Мы только превращаем висящий
                // eager-candidate в обычный последний зубец гребня. Ниже цикл будет забивать и
                // откатывать на общих основаниях.
                // - сначала подчистит свободный хвост после зубца;
                // - потом, если забои еще остались, разрушит сам зубец.
                self._comb.push(candidate._prong);
            }
        }   // match

        // ---------------------------------------------------------------------
        // Шаг 2. Основной цикл забоев справа налево:
        // - сначала подчистит свободный хвост после зубца;
        // - потом, если забои еще остались, разрушит сам зубец.
        // ---------------------------------------------------------------------
        while requested_erase > 0 && !self._cutting_board.is_empty() {
            // Нужно стирать и есть что стирать.

            // Длина доски до стирания
            let current_cb_len = self._cutting_board.len();

            // Проверить, покрывается ли хвост cutting_board последним зубом.
            let cb_tail_is_covered_by_last_prong = match self._comb.last() {
                Some(prong) => prong._cb_end >= current_cb_len,
                None => false,
            };

            if cb_tail_is_covered_by_last_prong {
                // =============================================================
                // Случай А. Правый край покрыт последним зубцом.
                // Значит, свободного хвоста справа уже нет, и забои пришли
                // непосредственно в подстановку.
                // =============================================================

                // Вынуть зубец из массива.
                let mut prong = self._comb.pop()
                    .expect("cb_tail_is_covered_by_last_prong implies non-empty comb");

                debug_assert_eq!(prong._cb_end, current_cb_len);
                debug_assert_eq!(prong._fb_end, self._franken_board.len());

                // 1. Дополнительный rollback до разрушения replacement-а.
                if let Some(rb) = prong._rollback_fn {
                    rb(self, &mut prong, WhenApplied::Before);
                }

                // 2. Полностью стереть replacement из franken_board.
                //
                // Здесь стираем весь замещающий текст целиком, даже если
                // заказанные забои покрывают только часть сырого ключа.
                // Это ключевая идея алгоритма:
                // частичное разрушение зубца = полный откат подстановки
                // + восстановление surviving raw tail.

                // replacement_len == 0 - легальный случай пустой подстановки.
                let replacement_len = self._franken_board.len() - prong.fb_start;
                if replacement_len > 0 {
                    self._franken_board.truncate(prong.fb_start);
                    self._screen_transfer_vec
                        .push(ScreenTransfer::Backspace(replacement_len));
                }

                // 3. Списать заказанные забои с сырого текста зубца.
                let prong_raw_len = prong._cb_end - prong.cb_start;
                let erase_now = requested_erase.min(prong_raw_len);

                let new_cb_len = current_cb_len - erase_now;
                self._cutting_board.truncate(new_cb_len);
                requested_erase -= erase_now;

                // 4. Восстановить surviving raw tail.
                //
                // Если зубец разрушен не полностью, то после удаления части сырого ключа
                // надо вернуть оставшийся кусок во franken_board.
                //
                // Берем текст от начала зубца до нового конца cutting_board и добавляем
                // во franken_board. После этого справа, возможно, образуется свободный хвост.
                if new_cb_len > prong.cb_start {
                    let tail_slice = &self._cutting_board[prong.cb_start..];

                    // Добавляем срез напрямую без лишних итераторов
                    self._franken_board.extend_from_slice(tail_slice);

                    // String собираем исключительно для команды ScreenTransfer
                    let surviving_text: String = tail_slice.iter().collect();
                    self._screen_transfer_vec.push(ScreenTransfer::Text(surviving_text));
                }

                // 5. Дополнительный rollback после восстановления сырого хвоста.
                if let Some(rb) = prong._rollback_fn {
                    rb(self, &mut prong, WhenApplied::After);
                }

            } else {
                // =============================================================
                // Случай Б. Справа есть свободный хвост, не покрытый зубцом.
                // Его текст одинаков и в cutting_board, и во franken_board.
                // =============================================================

                // Свободный хвост начинается сразу после последнего зубца.
                // Если зубцов нет, хвост начинается с нуля.
                let (free_tail_cb_start, free_tail_fb_start) = match self._comb.last() {
                    Some(prong) => (prong._cb_end, prong._fb_end),
                    None => (0, self._fb_rubicon),
                };

                let free_tail_cb_len = self._cutting_board.len() - free_tail_cb_start;
                let free_tail_fb_len = self._franken_board.len() - free_tail_fb_start;

                // Если сюда попали, свободный хвост должен существовать.
                debug_assert!(
                    free_tail_cb_len > 0,
                    "Infinite loop protection triggered: comb does not cover the board, but free tail length is 0"
                );

                // В release-сборке на всякий случай выходим, чтобы не зациклиться.
                #[cfg(not(debug_assertions))]
                if free_tail_cb_len == 0 {
                    break;
                }

                let erase_now = requested_erase.min(free_tail_cb_len);

                self._cutting_board
                    .truncate(self._cutting_board.len() - erase_now);

                self._franken_board
                    .truncate(self._franken_board.len() - erase_now);

                self._screen_transfer_vec
                    .push(ScreenTransfer::Backspace(erase_now));

                requested_erase -= erase_now;
            }
        }   // while
    }   // _apply_gboard_erase()

    /// Очищает разделочную доску при стабилизации потока.
    /// Весь текст уже отправлен на экран и отражен во franken_board.
    fn _mark_gboard_stabilization(&mut self) {
        self._cutting_board.clear();
        self._comb.clear();
        self._fb_rubicon = self._franken_board.len();
        self._prune_if_needed();
    }   // _mark_gboard_stabilization()

    /// Проверяет, стоит ли пробельный символ непосредственно слева от границ зубца
    /// одновременно в обеих досках.
    ///
    /// Двойная проверка нужна потому, что слева от кандидата может находиться
    /// уже состоявшаяся подстановка. Тогда символ в cutting_board — это хвост
    /// сырого ключа, а символ во franken_board — хвост replacement-а,
    /// и они не совпадают.
    ///
    /// # Параметры
    /// - `prong`: зубец, левые границы которого проверяются.
    ///
    /// # Возвращает
    /// `true`, если слева в обеих досках стоит пробельный символ.
    pub(super) fn _has_whitespace_before_candidate(&self, prong: &Prong) -> bool {

        if prong.cb_start == 0 || prong.fb_start == 0 {
            return false;
        }   // if

        let cb_char = self._cutting_board.get(prong.cb_start - 1);
        let fb_char = self._franken_board.get(prong.fb_start - 1);

        match (cb_char, fb_char) {
            (Some(cb), Some(fb)) => cb.is_whitespace() && fb.is_whitespace(),
            _ => false,
        }   // match
    }   // _has_whitespace_before()

    /// Если взведен флаг `capitalize_next_word`, делает первую букву в тексте заглавной
    /// и сбрасывает флаг. Если букв в тексте нет (пробелы, пунктуация), возвращает текст
    /// без изменений и оставляет флаг взведенным.
    fn _capitalize_if_needed(&mut self, text: &str) -> String {
        if self.flags.capitalize_next_word {
            if text.chars().any(|c| c.is_alphabetic()) {
                let mut chars = Vec::new();
                let mut capitalized = false;

                for c in text.chars() {
                    if !capitalized && c.is_alphabetic() {
                        chars.extend(c.to_uppercase());
                        capitalized = true;
                    } else {
                        chars.push(c);
                    }
                }

                self.flags.capitalize_next_word = false;
                return chars.into_iter().collect();
            }
        }
        text.to_string()
    }   // _capitalize_if_needed()

    /// Clears all state.
    fn _clear_all(&mut self) {
        self._cutting_board.clear();
        self._franken_board.clear();
        self._fb_rubicon = 0;
        self._comb.clear();
        self.flags._clear();
        self._candidate_position = _CandidatePosition::_None;
    }   // _clear_all()

    /// Ограничивает рост доски Франкенштейна.
    ///
    /// Оставляет хвост длиной _MAX_BOARD_CAPACITY для нужд
    /// голосовых команд (например, "стереть слово"), которым нужен
    /// левый контекст на экране.
    fn _prune_if_needed(&mut self) {
        let len = self._franken_board.len();
        if len > _MAX_BOARD_CAPACITY {
            let drop_count = len - _MAX_BOARD_CAPACITY;
            self._franken_board.drain(..drop_count);
            self._fb_rubicon = self._fb_rubicon.saturating_sub(drop_count);
        }
    }   // _prune_if_needed()
}   // impl SurgeTable

impl Drop for SurgeTable {
    fn drop(&mut self) {
        // future cleanup
    }   // drop()
}   // impl Drop for SurgeTable