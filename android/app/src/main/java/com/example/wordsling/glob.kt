/**
 * Глобальные переменные приложения
 */
package com.example.wordsling

// Порог числа символов в поле ввода, после которого разрешается очистка
const val INPUT_CLEAR_THRESHOLD = 300

// Порог молчания (мс), после которого инициируется очистка поля ввода
const val INPUT_SILENCE_THRESHOLD = 3000L

// Неочищаемый остаток (минимальное число символов, сохраняемое при очистке)
const val INPUT_CLEAR_KEEP_SIZE = 50

// Настройки TCP подключения к ПК-серверу
const val SERVER_IP = "192.168.0.172"
const val SERVER_PORT = 51234
const val RECONNECT_DELAY_MS = 3000L
