package com.example.wordsling.gboard

import android.accessibilityservice.AccessibilityService
import android.annotation.SuppressLint
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import com.example.wordsling.gboardControl

/**
 * AccessibilityService приложения wordsling.
 *
 * Тонкая обёртка над Android AccessibilityService. Не содержит логики управления
 * клавиатурой — вся логика живёт в GboardKeeper.
 *
 * ОТВЕТСТВЕННОСТЬ:
 * - Публикация ссылки на себя в глобальную переменную wordslingAccessibilityService,
 *   чтобы GboardControl мог обращаться к API AccessibilityService (в частности,
 *   к windows[] и корневым узлам окон).
 * - Ретрансляция событий Accessibility (изменения окон gboard) в GboardKeeper.
 *
 * # Жизненный цикл
 * Сервис запускается системой Android только после того, как пользователь явно
 * включит его в настройках спец. возможностей. Готов к работе не сразу при создании,
 * а только после вызова onServiceConnected() системой.
 */
@SuppressLint("AccessibilityPolicy")
class WordslingAccessibilityService : AccessibilityService() {

    // Тег для логирования
    private val _TAG = "wordsling.AccSvc"

    /**
     * Вызывается системой после успешной привязки сервиса.
     *
     * До этого момента AccessibilityService создан, но API (windows,
     * getRootInActiveWindow и т.п.) ещё не готов к использованию. Именно
     * здесь можно начинать реальную работу.
     *
     * Побочные эффекты:
     * - устанавливает глобальную ссылку wordslingAccessibilityService на
     *   этот экземпляр.
     */
    override fun onServiceConnected() {
        super.onServiceConnected()
        Log.i(_TAG, "onServiceConnected()")

        // === Настройка доступа к содержимому окон ===
        val currentInfo = this.serviceInfo
        if (currentInfo != null) {
            val hasFlag = (currentInfo.flags and android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS) != 0

            if (!hasFlag) {
                Log.w(_TAG, "Флаг FLAG_RETRIEVE_INTERACTIVE_WINDOWS отсутствует. Устанавливаем программно.")

                // Модифицируем текущие настройки и применяем их
                currentInfo.flags = currentInfo.flags or android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
                this.serviceInfo = currentInfo

                // Проверяем, применился ли флаг
                val updatedFlags = this.serviceInfo?.flags ?: 0
                if ((updatedFlags and android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS) != 0) {
                    Log.i(_TAG, "Флаг FLAG_RETRIEVE_INTERACTIVE_WINDOWS успешно установлен.")
                } else {
                    Log.e(_TAG, "Попытка установить флаг не удалась (возможно, требуется указать canRetrieveWindowContent=\"true\" в XML конфигурации).")
                }
            } else {
                Log.i(_TAG, "Флаг FLAG_RETRIEVE_INTERACTIVE_WINDOWS уже присутствует.")
            }
        } else {
            Log.e(_TAG, "serviceInfo == null, невозможно настроить сервис!")
        }

        // Публикуем контроллер для глобального доступа
        gboardControl = GboardControl(this)
    } // onServiceConnected()

    /**
     * Приходят события UI, в частности от окна gboard.
     *
     * Пока не обрабатываются. В будущем здесь будет фильтрация событий
     * от пакета com.google.android.inputmethod.latin и передача их
     * в GboardKeeper для быстрой реакции на изменение состояния микрофона.
     *
     * ВАЖНО: этот метод вызывается системой очень часто, любая тяжёлая работа
     * здесь недопустима. Только диспетчеризация.
     */
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // TODO: фильтрация событий gboard и передача в GboardKeeper.
        // event?.packageName == "com.google.android.inputmethod.latin"
    } // onAccessibilityEvent()

    /**
     * Вызывается системой при принудительной остановке сервиса.
     *
     * По документации Android — реакция на «прерывание» работы сервиса, например,
     * при переключении обработчика жестов. На практике вызывается редко.
     * Оставлено пустым, полное освобождение ресурсов происходит в onDestroy().
     */
    override fun onInterrupt() {
        Log.w(_TAG, "onInterrupt()")
    } // onInterrupt()

    /**
     * Уничтожение сервиса.
     *
     * Вызывается системой при отключении сервиса пользователем или при завершении
     * процесса приложения.
     *
     * Побочные эффекты:
     * - сбрасывает глобальную ссылку wordslingAccessibilityService в null,
     *   чтобы GboardKeeper/GboardControl не пытались работать с мёртвым сервисом.
     */
    override fun onDestroy() {
        Log.i(_TAG, "onDestroy()")
        gboardControl = null
        super.onDestroy()
    } // onDestroy()

} // WordslingAccessibilityService