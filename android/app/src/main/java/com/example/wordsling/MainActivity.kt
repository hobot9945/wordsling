package com.example.wordsling

import android.content.Context
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.TextWatcher
import android.text.method.ScrollingMovementMethod
import android.view.inputmethod.BaseInputConnection
import android.widget.EditText
import android.widget.TextView
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.example.wordsling.databinding.ActivityMainBinding
import android.view.inputmethod.InputMethodManager
import androidx.lifecycle.lifecycleScope

/**
 * Главный экран приложения.
 *
 * Отвечает за:
 * - прием текста из `EditText`;
 * - передачу текущего состояния ввода в `TextProcessor`;
 * - отображение дельты в тестовом журнале;
 * - отложенную подчистку поля ввода после периода молчания.
 *
 * Экран пока работает как локальный отладочный стенд:
 * сетевая отправка здесь не выполняется, журнал нужен только
 * для визуальной проверки работы протокола дельт.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding

    // Элементы UI
    private lateinit var _inputEditText: EditText
    private lateinit var _logTextView: TextView

    // Выделение текстовой добавки
    private lateinit var _textProcessor: TextProcessor

    // TCP-клиент для отправки дельт на ПК
    private lateinit var _tcpClient: TcpClient

    // Хэндлер для отложенных задач (очистка поля)
    private val _handler = Handler(Looper.getMainLooper())
    private val _clearInputRunnable = Runnable { _clearInputField() }

    // Флаг программного изменения текста, чтобы блокировать TextWatcher при очистке
    private var _isProgrammaticallyChangingText = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Инициализируем объекты в нужном порядке
        binding = ActivityMainBinding.inflate(layoutInflater)
        _inputEditText = binding.inputEditText
        _logTextView = binding.logTextView
        _textProcessor = TextProcessor()
        _tcpClient = TcpClient(this, lifecycleScope)  // Передаем родной scope приложения, он завершится
                                                // в случае остановки и в корутинах случится исключение.
        _tcpClient.start()

        // Включаем программную прокрутку для журнала
        _logTextView.movementMethod = ScrollingMovementMethod.getInstance()

        // Разрисовываем экран
        setContentView(binding.root)
        ViewCompat.setOnApplyWindowInsetsListener(binding.main) { v, insets ->
            val insetsType = WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime()
            val paddingInsets = insets.getInsets(insetsType)
            v.setPadding(paddingInsets.left, paddingInsets.top, paddingInsets.right, paddingInsets.bottom)
            insets
        }

        // Автоматический вывод клавиатуры при старте. Замыкание ставится в очередь, будет выполнено
        // после разрисовки экрана.
        _inputEditText.post {
            _inputEditText.requestFocus()
            val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
            imm.showSoftInput(_inputEditText, 0)
        } // post

        // Установить слушателя изменений текста в поле ввода
        _inputEditText.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {
                // Не используется
            }

            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                // Не используется
            }

            override fun afterTextChanged(s: Editable?) {

                // Блокируем обработку при программном изменении (очистке)
                if (_isProgrammaticallyChangingText) return

                // Пустота не должна случаться, отсеиваем на всякий случай.
                s ?: return

                val delta = _textProcessor.takeText(s).delta

                // Применяем дельту к журналу, если она есть
                if (delta.isNotEmpty()) {
                    _applyDelta(_logTextView, delta)
                    _tcpClient.send(delta)
                }

                // Управление таймером очистки
                if (s.length > INPUT_CLEAR_THRESHOLD) {
                    _handler.removeCallbacks(_clearInputRunnable)
                    _handler.postDelayed(_clearInputRunnable, INPUT_SILENCE_THRESHOLD)
                } else {
                    _handler.removeCallbacks(_clearInputRunnable)
                }
            }
        })
    } // onCreate()

    override fun onDestroy() {
        super.onDestroy()

        // Легально закрыть TcpClient. Он и сам остановил бы свои корутины, когда умерла бы
        // lifecycleScope, но так приличнее.
        _tcpClient.stop()
    } // onDestroy()

    /**
     * Подчищает поле ввода, сохраняя хвост текста по границе слова.
     *
     * Очистка нужна, чтобы длинный буфер `EditText` не рос бесконечно
     * во время непрерывного ввода.
     *
     * Алгоритм:
     * - не выполняет очистку, если IME еще держит активную композицию;
     * - не выполняет очистку, если текст не достиг порога `INPUT_CLEAR_THRESHOLD`;
     * - вычисляет позицию разреза так, чтобы сохранить не менее
     *   `INPUT_CLEAR_KEEP_SIZE` символов;
     * - сдвигает границу влево до ближайшего пробельного символа,
     *   чтобы не разрывать слово;
     * - удаляет начальную часть текста, очищает тестовый журнал
     *   и синхронизирует `TextProcessor` с новым остатком.
     *
     * Побочные эффекты:
     * - изменяет содержимое `_inputEditText`;
     * - изменяет содержимое `_logTextView`;
     * - сбрасывает внутреннее состояние `_textProcessor`.
     */
    private fun _clearInputField() {

        // Если текст находится в стадии композиции (gboard еще не зафиксировал слово),
        // очистку не производим. Она сработает в следующий период молчания.
        if (_textProcessor.compositionFlag) return

        val currentText = _inputEditText.text.toString()

        // Если текст меньше минимума, очистка не требуется
        if (currentText.length <= INPUT_CLEAR_THRESHOLD) return

        // Вычисляем базовую позицию разреза: отступаем INPUT_CLEAR_KEEP_SIZE символов от конца.
        var cutIndex = currentText.length - INPUT_CLEAR_KEEP_SIZE

        // Идем к началу строки (влево) в поисках ближайшего пробела, чтобы не разорвать слово.
        // Так как мы идем влево, остаток текста будет РАСТИ, что гарантирует "не менее 50 символов".
        while (cutIndex > 0 && !currentText[cutIndex].isWhitespace()) {
            cutIndex--
        } // while

        // Если нашли пробел, сдвигаем индекс на 1 вправо, чтобы сам пробел удалился,
        // и оставшийся в поле текст не начинался с пробела.
        if (cutIndex > 0) {
            cutIndex++
        } // if

        if (cutIndex > 0) {
            // Флаг выставляется ДО очистки. В процессе очистки мы ныряем в afterTextChanged()
            // и флаг должен выкинуть нас оттуда.
            _isProgrammaticallyChangingText = true

            // Завершаем композитный ввод, чтобы клавиатура не попыталась отменить очистку
            BaseInputConnection(_inputEditText, false).finishComposingText()

            // Удаляем символы по месту, не ломая буфер IME
            _inputEditText.text.delete(0, cutIndex)
            _inputEditText.setSelection(_inputEditText.text.length)

            // Очистить поле журнала.
            _logTextView.text = ""

            // Сбрасываем процессор на новый опорный текст
            _textProcessor.reset(_inputEditText.text.toString())

            _isProgrammaticallyChangingText = false
        } // if
    } // _clearInputField()

    /**
     * Применяет строку дельты к текущему тексту тестового журнала.
     *
     * Метод нужен только для локальной визуальной проверки того,
     * как `TextProcessor` кодирует изменения текста.
     *
     * Поддерживаемые форматы:
     * - `текст` — дописать текст в конец;
     * - `[N]текст` — удалить `N` символов с конца, затем дописать текст;
     * - `*[N]текст` — то же самое, но с маркером стабилизации `*`,
     *   который переносится в журнал как обычный символ.
     *
     * Если строка дельты пуста, метод ничего не делает.
     *
     * @param logTextView виджет, в котором отображается тестовый журнал.
     * @param delta строка дельты в формате протокола `wordsling`.
     *
     * Побочные эффекты:
     * - изменяет текст `logTextView`;
     * - прокручивает журнал к последней строке.
     */
    private fun _applyDelta(logTextView: TextView, delta: String) {
        if (delta.isEmpty()) return

        var currentText = logTextView.text.toString()
        var remainder = delta
        var prefix = ""

        // Извлекаем маркер стабилизации, переносим "как есть"
        if (remainder.startsWith("*")) {
            prefix = "*"
            remainder = remainder.substring(1)
        }

        // Ищем и применяем команду удаления [N]
        if (remainder.startsWith("[")) {
            val closeBracketIndex = remainder.indexOf(']')
            if (closeBracketIndex != -1) {
                val countStr = remainder.substring(1, closeBracketIndex)
                val deleteCount = countStr.toIntOrNull() ?: 0
                val textToAdd = remainder.substring(closeBracketIndex + 1)

                // Удаляем символы с конца
                currentText = if (currentText.length >= deleteCount) {
                    currentText.dropLast(deleteCount)
                } else {
                    ""
                }
                remainder = textToAdd
            } // if
        } // if

        // Формируем и устанавливаем итоговый текст
        currentText += prefix + remainder
        logTextView.text = currentText

        // Прокрутка в конец журнала
        val layout = logTextView.layout
        if (layout != null) {
            val scrollAmount = layout.getLineTop(logTextView.lineCount) - logTextView.height
            if (scrollAmount > 0) {
                logTextView.scrollTo(0, scrollAmount)
            } else {
                logTextView.scrollTo(0, 0)
            } // if
        } // if
    } // _applyDelta()
} // MainActivity