package com.example.wordsling

import android.content.Context
import android.util.Log
import android.widget.Toast
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.InetSocketAddress
import java.net.Socket
import kotlin.time.Duration.Companion.milliseconds

/**
 * TCP-клиент для обмена данными с ПК-сервером.
 *
 * Обеспечивает асинхронное подключение, чтение и запись через Kotlin Coroutines.
 * Поддерживает авто-рекконект при обрыве связи.
 *
 * ОТВЕТСТВЕННОСТЬ:
 * - Управление жизненным циклом TCP-соединения.
 * - Потокобезопасная отправка строковых сообщений (дельт).
 * - Фоновое чтение входящего потока (пока без обработки).
 */
class TcpClient(
    private val context: Context,
    private val scope: CoroutineScope
) {
    // Канал для отправки строк в корутину записи
    private val _sendChannel = Channel<String>(Channel.UNLIMITED)

    // Флаг активной работы клиента. Выставляется извне, проверяется корутинами, чтобы понять, что
    // нужно завершать работу.
    private var _isRunning = false

    /**
     * Запускает процесс подключения и поддержания связи.
     *
     * При обрыве связи инициирует авто-рекконект с задержкой `RECONNECT_DELAY_MS`.
     */
    fun start() {
        if (_isRunning) return
        _isRunning = true

        scope.launch(Dispatchers.IO) {
            while (_isRunning) {
                try {
                    connectAndProcess()
                } catch (e: IOException) {
                    Log.e("TcpClient", "Connection cycle error: ${e.message}")
                } // catch

                if (_isRunning) {
                    _showToast("Нет соединения, переподключение...")
                    delay(RECONNECT_DELAY_MS.milliseconds)
                } // if
            } // while
        } // launch
    } // start()

    /**
     * Останавливает клиент и закрывает канал отправки.
     */
    fun stop() {
        _isRunning = false
        _sendChannel.close()
    } // stop()

    /**
     * Помещает строку в очередь на отправку.
     *
     * Вызов безопасен из любого потока.
     *
     * @param message строка для отправки на сервер.
     */
    fun send(message: String) {
        if (!_isRunning) return
        _sendChannel.trySend(message)
    } // send()

    /**
     * Устанавливает соединение и запускает циклы чтения/записи.
     *
     * Блокирует корутину до тех пор, пока соединение активно.
     * При ошибке чтения/записи выбрасывает исключение, что приводит к реконнекту.
     * Ресурсы сокета и потоков управляются автоматически через `use`.
     *
     * Используется `coroutineScope`, чтобы исключение в корутине чтения (`readJob`)
     * отменяло корутину записи и пробрасывалось наверх в `start()`, а не валило `lifecycleScope`.
     */
    private suspend fun connectAndProcess() = coroutineScope {
        Socket().use { socket ->
            socket.connect(InetSocketAddress(SERVER_IP, SERVER_PORT), 5000)
            _showToast("Подключено к серверу")

            OutputStreamWriter(socket.getOutputStream(), Charsets.UTF_8).use { writer ->
                BufferedReader(InputStreamReader(socket.getInputStream(), Charsets.UTF_8)).use { reader ->

                    // Корутина чтения. При обрыве связи или EOF выбрасывает IOException,
                    // что сворачивает весь `coroutineScope`.
                    val readJob = launch {
                        while (isActive && socket.isConnected) {
                            val line = reader.readLine() ?: throw IOException("Connection closed by server")
                            Log.d("TcpClient", "Received: $line")
                        } // while
                    } // launch

                    // Цикл записи. При ошибке записи исключение пробрасывается наверх,
                    // отменяя `readJob` и сворачивая `coroutineScope`.
                    for (message in _sendChannel) {
                        writer.write(message)
                        writer.flush()
                    } // for

                    readJob.cancelAndJoin()
                } // use (reader)
            } // use (writer)
        } // use (socket)
    } // connectAndProcess()

    /**
     * Показывает Toast на главном потоке.
     *
     * @param message текст сообщения.
     */
    private suspend fun _showToast(message: String) {
        withContext(Dispatchers.Main) {
            Toast.makeText(context, message, Toast.LENGTH_SHORT).show()
        } // withContext
    } // _showToast()

} // TcpClient