package com.example.wordsling

import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.widget.EditText
import android.widget.TextView
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.example.wordsling.databinding.ActivityMainBinding

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding

    // Элементы UI
    private lateinit var _inputEditText: EditText
    private lateinit var _logTextView: TextView

    // Выделение текстовой добавки
    private lateinit var _textProcessor: TextProcessor

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Инициализируем объекты в нужном порядке
        binding = ActivityMainBinding.inflate(layoutInflater)
        _inputEditText = binding.inputEditText
        _logTextView = binding.logTextView
        _textProcessor = TextProcessor()

        // Разрисовываем экран
        setContentView(binding.root)
        ViewCompat.setOnApplyWindowInsetsListener(binding.main) { v, insets ->
            val insetsType = WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime()
            val paddingInsets = insets.getInsets(insetsType)
            v.setPadding(paddingInsets.left, paddingInsets.top, paddingInsets.right, paddingInsets.bottom)
            insets
        }

        // Слушатель изменений текста в поле ввода
        _inputEditText.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {
                // Не используется
            }

            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                // Не используется
            }

            override fun afterTextChanged(s: Editable?) {

                // Пустота не должна случаться, отсеиваем на всякий случай.
                s ?: return

                val delta = _textProcessor.takeText(s).delta

                // Добавляем дельту в журнал, если она есть
                if (delta.isNotEmpty()) {
                    _applyDelta(_logTextView, delta)
                }
            }
        })
    } // onCreate()

    /// Применяет строку дельты к текущему тексту журнала.
    ///
    /// Тестовая функция, нужна, чтобы посмотреть, правильно ли работает дельта.
    ///
    /// Разбирает форматы вида `*[N]текст`, `[N]текст` или `текст`.
    /// Сначала удаляет `N` символов с конца текущего текста журнала,
    /// затем добавляет `*` (если она была в начале) и оставшийся `текст`.
    ///
    /// - параметры:
    ///   - logTextView: элемент UI журнала
    ///   - delta: строка дельты для применения
    /// - побочные эффекты:
    ///   - изменяет текст в переданном logTextView
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

    } // _applyDelta()
} // MainActivity