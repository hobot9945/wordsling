/**
 * Глобальные переменные приложения
 */
package com.example.wordsling

import android.annotation.SuppressLint
import android.content.Context
import com.example.wordsling.gboard.GboardControl
import kotlinx.coroutines.channels.Channel

/*
  ======================================== Константы ===========================================
*/

// Порог числа символов в поле ввода, после которого разрешается очистка
const val INPUT_CLEAR_THRESHOLD = 512

// Порог молчания (мс), после которого инициируется очистка поля ввода
const val INPUT_SILENCE_THRESHOLD = 3000L

// Неочищаемый остаток (минимальное число символов, сохраняемое при очистке)
const val INPUT_CLEAR_KEEP_SIZE = 50

// Интервал опроса состояния микрофона (мс)
const val MIC_POLL_INTERVAL_MS = 100L

// Настройки TCP подключения к ПК-серверу
const val SERVER_IP = "192.168.0.172"
const val SERVER_PORT = 51234
const val RECONNECT_DELAY_MS = 3000L

/*
  ====================================== Общие переменные ======================================
*/

/**
 * Ссылка на GboardControl.
 *
 * Заполняется в классе wordslingAccessibilityService сервисом в onServiceConnected() и
 * сбрасывается в onDestroy().
 * Используется GboardKeeper для доступа к дереву окон клавиатуры.
 *
 * null означает, что сервис ещё не подключен системой (пользователь не включил его в настройках спец.
 * возможностей).
 */
@SuppressLint("StaticFieldLeak")
@Volatile
var gboardControl: GboardControl? = null

/**
 * Канал команд управления клавиатурой, направленных в GboardKeeper.
 * Продюсеры: MainActivity, TcpClient. Потребитель: GboardKeeper.
 * Буферизованный: сообщения не теряются, если keeper ещё не начал читать.
 */
val channelToKeeper = Channel<KeeperSignal>(Channel.BUFFERED)
enum class KeeperSignal {
    // От главной активности
    GBOARD_STARTED,             // приложение запустило клавиатуру - надо включить микрофон
    INPUT_EDITTEXT_ACTIVITY,    // активность в поле ввода - надо сбросить таймер молчания
    INPUT_EDITTEXT_CLEARED,     // поле ввода очищено - микрофон отключен, надо запустить

    SWITCH_LANGUAGE,
}

/// Инициализировать модуль.
fun initializeGlob(context: Context) {

}