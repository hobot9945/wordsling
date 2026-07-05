package com.example.wordsling

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.example.wordsling.databinding.ActivityMainBinding

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Инициализируем биндинг
        binding = ActivityMainBinding.inflate(layoutInflater)

        // Разрисовываем экран
        setContentView(binding.root)
        ViewCompat.setOnApplyWindowInsetsListener(binding.main) { v, insets ->
            val insetsType = WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime()
            val paddingInsets = insets.getInsets(insetsType)
            v.setPadding(paddingInsets.left, paddingInsets.top, paddingInsets.right, paddingInsets.bottom)
            insets
        }

        // Пример использования (потом удалишь):
        binding.logTextView.text = "Программа запущена"
    }
}