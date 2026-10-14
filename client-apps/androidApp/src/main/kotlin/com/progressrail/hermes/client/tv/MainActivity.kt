package com.progressrail.hermes.client.tv

import android.app.Activity
import android.os.Bundle
import android.view.Gravity
import android.widget.TextView

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setContentView(TextView(this).apply {
            gravity = Gravity.CENTER
            text = getString(R.string.app_name)
            textSize = 24f
        })
    }
}