/// Этот класс отвечает за взаимодействие с gboard - проверка состояния икон микрофона и
// переключения языка, тапы по этим иконам.
//
// # Детали
// Проверка состояния икон включает в себя их поиск в окне клавиатуры. В общем случае поиск нужного
// узла, например, иконы микрофона включает в себя рекурсивный перебор всех узлов окна.  В случае
// повторного поиска возможна оптимизация, можно запомнить путь к узлу и добираться к нему без
// перебора узлов окна. Нужно только учесть что, например, для разных состояний микрофона
// окно выглядит по-разному, значит и пути к узлу микрофона будут разными. То же самое касается
// Иконы смены языка. Если окно не перестраивалось, что бывает только после тапа, будет действителен
// ранее найденный узел, вообще можно обойтись без поиска.
package com.example.wordsling.gboard

import android.accessibilityservice.AccessibilityService
import android.os.Handler
import android.os.Looper
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.util.Log
class GboardControl(private val _accService: AccessibilityService) {
    private val _TAG = "wordsling.GbCtlr"

    private val _handler = Handler(Looper.getMainLooper())

    /// Список путей к узлам. Каждый путь - список индексов узлов в дереве. Нулевой и элемент - индекс
    /// окна, первый - индекс узла первого уровня, второй - индекс узла второго и т.д.
    /// Это кэш для путей к узлу микрофона.
    private val _micNodePathCache: MutableList<List<Int>> = mutableListOf();

    /// Это кэш для путей к узлу языка.
    private val _langNodePathCache: MutableList<List<Int>> = mutableListOf();

    ///     Определяет состояние микрофонa.
    ///
    /// # Детали
    /// для определения состояния микрофона сначала ищется узел микрофона, потом читаем его
    /// описание, по нему определяем включен микрофон или выключен. Не стоит беспокоиться о том,
    /// что выполняется лишний поиск узла. Найденный узел запоминается и последующие в поиски будут
    /// происходить очень быстро. Узел будет сброшен только после тапа, но даже при поиске узла перед
    /// тапом он будет взят из сохранённой переменной.
    /// Из-за реализации клавиатуры возможно краткие моменты когда поиск узла будет неудачным.
    /// В этом случае возвращается null.
    fun getMicState(): MicState? {
        val node = _searchMicNode()
        return when (node?.description) {

            MicState.MIC_ON.signature[0], MicState.MIC_ON.signature[1] ->
                MicState.MIC_ON

            MicState.MIC_OFF.signature[0], MicState.MIC_OFF.signature[1] ->
                MicState.MIC_OFF

            else -> null
        }
    }
    enum class MicState(val signature: List<String>) {
        MIC_ON(listOf("Завершить голосовой ввод", "Stop voice typing")),
        MIC_OFF(listOf("Включить голосовой ввод", "Use voice typing")),
    }

    ///     Определяет текущий язык.
    ///
    /// # Детали
    /// То же самое что и для предыдущей функции.
    fun getCurrentLanguage(): CurrentLanguage? {
        val node = _searchLangNode()
        return when (node?.description) {

            CurrentLanguage.ENGLISH.signature -> CurrentLanguage.ENGLISH

            CurrentLanguage.RUSSIAN.signature -> CurrentLanguage.RUSSIAN

            else -> null
        }
    }
    enum class CurrentLanguage(val signature: String) {
        ENGLISH("Next language"),
        RUSSIAN("Следующий язык"),
    }

    ///     Ищет икону микрофона и кликает по ней.
    ///
    /// # Детали
    /// Если клик был успешным, то всё хорошо, если нет, то клик повторяется до пяти раз с
    /// интервалом 100 мс.
    ///
    /// # Аргументы
    /// * firstDelayMs - выдержка перед первым запуском. Нужна, например, после переключения языка.
    /// Если выполнять тап немедленно, он гарантированно не срабатывает.
    fun searchAndTapMicIcon(firstDelayMs: Int = 0) {

        if (firstDelayMs == 0) {
            _recursivePlanner(::_searchAndTapMicIcon, 100,5)
        } else {
            _handler.postDelayed({ _recursivePlanner(::_searchAndTapMicIcon, 100,5) },
                firstDelayMs.toLong())
        }
    }

    ///     Ищет икону языка и кликает по ней.
    /// В остальном всё так же как и в предыдущей функции.
    ///
    /// # Аргументы
    /// * firstDelayMs - выдержка перед первым запуском.
    fun searchAndTapLangIcon(firstDelayMs: Int = 0) {

        if (firstDelayMs == 0) {
            _recursivePlanner(::_searchAndTapLangIcon, 100,5)
        } else {
            _handler.postDelayed({ _recursivePlanner(::_searchAndTapLangIcon, 100,5) },
                firstDelayMs.toLong())
        }
    }

    ///     Ищет икону языка и кликает по ней.
    ///
    /// # Возвращает
    /// успех/неудача
    private fun _searchAndTapMicIcon(): Boolean {
        val success = _searchMicNode()?.node?.performAction(AccessibilityNodeInfo.ACTION_CLICK)?: false
        if (!success) {
            Log.e(_TAG, "Не выполнен тап по иконе микрофона.")
        }

        return success
    }

    ///     Ищет икону языка и кликает по ней.
    ///
    /// # Возвращает
    /// успех/неудача
    private fun _searchAndTapLangIcon(): Boolean {

        val success = _searchLangNode()?.node?.performAction(AccessibilityNodeInfo.ACTION_CLICK)?: false
        if (!success) {
            Log.e(_TAG, "Не выполнен тап по иконе языка.")
        }

        return success
    }

    ///     Ищет узел микрофона.
    ///
    /// # Детали
    /// Сначала проверяет если действительный результат прошлого поиска. В случае неудачи проходит
    /// по путём, сохранённым в кэше. В случае неудачи выполняет полный поиск по окну, корректируя
    /// кэши.
    private fun _searchMicNode(): Node? {

        // Ищем через кэше путей.
        var micNode = _searchNodeInCache(_MIC_NODE_SIGNATURE, _micNodePathCache)
//Log.d(_TAG, "Найдено через кэш путей: ${micNode?.description}")
        if (micNode != null) {
            // Найдено через кэш.
            return micNode
        } else {
            // Через кэш не найдено, выполняем полный обход дерева окна, попутно заполняем кэши.
            micNode = _searchNode(_MIC_NODE_SIGNATURE)
//Log.d(_TAG, "Результат поиска по окну: ${micNode?.description}")

            if (micNode != null) {

                // Добавить в _micNodePathCache
                _addPathToCache(_winInd, _reversedNodePathList, _micNodePathCache)
            }

            return micNode
        }
    }
    /// Варианты содержимого поля contentDescription узла микрофона. По ним ищем узел.
    private val _MIC_NODE_SIGNATURE = listOf("голосовой ввод", "voice typing");

    ///     Ищет узел языка.
    ///
    /// # Детали
    ///
    private fun _searchLangNode(): Node? {

        // Ищем в кэше путей.
        var langNode = _searchNodeInCache(_LANG_NODE_SIGNATURE, _langNodePathCache)
//Log.d(_TAG, "Найдено через кэш путей: ${langNode?.description}")
        if (langNode != null) {
            // Найдено через кэш.
            return langNode
        } else {
            // Через кэш не найдено, выполняем полный обход дерева окна, попутно заполняем кэши.
            langNode = _searchNode(_LANG_NODE_SIGNATURE)
//Log.d(_TAG, "Результат поиска по окну: ${langNode?.description}")

            if (langNode != null) {
                // Добавить в _langNodePathCache
                _addPathToCache(_winInd, _reversedNodePathList, _langNodePathCache)
            }

            return langNode
        }
    }
    /// Варианты содержимого поля contentDescription узла переключения языка. По ним ищем узел.
    private val _LANG_NODE_SIGNATURE = listOf("язык", "language");

    ///     По всем путям кэша ищет узел, подходящий под сигнатуру.
    /// Возвращает либо найденный узел с его описанием, либо null. При поиске проходит по всем
    /// наследникам корневого узла до конца. Возможен выход за границы списка детей, в этом случае
    /// поиск считается неудачным.
    private fun _searchNodeInCache(signature: List<String>, cache: List<List<Int>>): Node? {
        for (path in cache) {
            try {
                // Взять корневой узел
                var node = _accService.windows[path[0]].root    // корневой

                // Пройти к ребенку, внуку и т.д. до конца
                for (childInd in path.subList(1, path.size)) {
                    node = node.getChild(childInd)
                }

                val nodeDesc = (node.contentDescription?: "").toString()
                if (signature.any { nodeDesc.contains(it) }) {    // контент узла найден в сигнатуре?
                    return Node(node, (node.contentDescription ?: "").toString())
                }
            } catch (_: Exception) {
//Log.e(_TAG, "$e")
                continue
            }
        }

        return null
    }

    /// Ищет узел с заданной сигнатурой.
    //  Ищет окно клавиатуры и запускает рекурсивный поиск узла.
    /// # Аргументы
    /// * node - начальный узел
    /// * signature - список строк, которые могут содержаться в поле contentDescription искомого узла.
    ///
    /// # Возвращает
    /// найденный узел, либо null, если узла с такой сигнатурой нет в дереве.
    private fun _searchNode(signature: List<String>): Node? {
//val startTime = System.nanoTime()
//try {
        _reversedNodePathList.clear()
        val rootNodeOfKeyboardWindow = _searchRootNodeOfKeyboardWindow()
        if (rootNodeOfKeyboardWindow != null) {
            val foundNode = _searchNodeRecursively(rootNodeOfKeyboardWindow, signature)
            if (foundNode != null) {
                return foundNode
            }
        }

        return null
//} finally {
//    val durationMs = (System.nanoTime() - startTime) / 1000000
//    Log.d(TAG, "_найтиУзелМикрофона() выполнился за ${durationMs} мс")
//}
    }
    /// Сюда будем запоминать путь к узлу - индексы узла на каждом уровне дерева. Они записываются
    /// при возврате из рекурсии, поэтому в обратном порядке.
    private val _reversedNodePathList: MutableList<Int> = mutableListOf()
    /// Сюда запишем индекс окна.
    private var _winInd: Int = 0;

    ///     Побочным эффектом добавляет путь (перевёрнутый список индексов узлов в дереве) в кэш -
    /// список таких путей.
    ///
    /// # Детали
    /// добавляется в начало списка кэша, если в результате длина кэша превышает 4, последние
    /// элементы удаляются, потому что валидны только четыре варианта пути.
    ///
    /// # Аргументы
    /// * winInd - индекс окна клавиатуры в списке окон
    /// * reverseNodePathList - перевёрнутый список индексов узлов на каждом уровне дерева
    ///
    /// # Возвращает
    /// побочным эффектом добавляется новый путь в список кэша.
    private fun _addPathToCache(winInd: Int, reversedNodePathList: MutableList<Int>,
                                nodePathCache: MutableList<List<Int>>)
    {
        // Сформировать список пути в нормальном порядке.
        val finalPath = listOf(winInd) + reversedNodePathList.reversed()

        // Вставить уже нормального вида путь в начало кэша.
        nodePathCache.add(0, finalPath)

        // Удалить все пути дальше четвертого. Больше держать нет смысла, так как всего четыре вариации
        // окна клавиатуры - два состояния микрофона, два состояния языка.
        if (nodePathCache.size > 4) {
            nodePathCache.subList(4, nodePathCache.size).clear()
        }
    }

    /// Проходит по дереву узлов окна клавиатуры и ищет узел с заданной сигнатурой. Напрямую никогда
    /// не вызывается, вызывается через оболочку _searchNode().
    /// # Аргументы
    /// * node - начальный узел
    /// * signature - список строк, которые могут содержаться в поле contentDescription искомого узла.
    ///
    /// # Возвращает
    /// найденный узел, либо null, если узла с такой сигнатурой нет в дереве.
    private fun _searchNodeRecursively(node: AccessibilityNodeInfo, signature: List<String>): Node? {

        // Проверить найден ли узел микрофона.
        val nodDesc = node.contentDescription?: ""
        if (signature.any {nodDesc.contains(it)}) {
//Log.i(_TAG,
//    "Найден узел микрофона: desc='${node.contentDescription}', id='${node.viewIdResourceName}'")

            return Node(node, (node.contentDescription?: "").toString())
        }

        // Обойти поддерево.
        for (i in 0 until node.childCount) {
            val childNode = node.getChild(i) ?: continue
            val foundNode = _searchNodeRecursively(childNode, signature)
//Log.d(_TAG, "i = '${i}', childNode.contentDescription = '${childNode.contentDescription}'")
            if (foundNode != null) {

                // Побочным эффектом запомнить индекс узла
                _reversedNodePathList.add(i)

                return foundNode
            }
        }
        return null
    }

    /// Проходит по всем окнам приложения, ищет окно клавиатуры, возвращает корневой узел или null,
    /// если не найдено.
    private fun _searchRootNodeOfKeyboardWindow(): AccessibilityNodeInfo? {
        _accService.windows.forEachIndexed { index, winInfo ->
            val rootNode = winInfo.root
            if (rootNode != null && rootNode.packageName == "com.google.android.inputmethod.latin" &&
                winInfo.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD)
            {
                // Побочным эффектом запомнить индекс окна
                _winInd = index
                return rootNode
            }
        }

        return null
    }

    ///     Запускает переданную параметром функцию. Если функция возвращает неуспех, её перезапуск
    /// планируется через некоторый тайм-аут. Если и он будет неуспешным, задание перепланируется снова,
    /// и так столько раз, сколько указано параметром.
    ///
    /// # Аргументы
    /// * func - исполняемая функция
    /// * execDelayMs - время, выжидаемое между перезапусками
    /// * execCount - максимальное количество попыток исполнения функции
    private fun _recursivePlanner(func: () -> Boolean, execDelayMs: Int, execCount: Int) {

        // Выполняем тап.
        val success = func()

        // Если тап не удался и счетчик не исчерпан, рекурсивно планируем следующий тап
        if (!success && execCount > 0) _handler.postDelayed({
            _recursivePlanner(func, execDelayMs, execCount - 1)
        }, execDelayMs.toLong())
    }
}

/// Для возврата из функций поиска узла.
data class Node(val node: AccessibilityNodeInfo, val description: String)