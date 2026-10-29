package io.github.tatselkrik.hrisemployee

internal object RustPortal {
    init { System.loadLibrary("hris_employee_android") }
    external fun start(profileDirectory: String): String
    external fun stop()
}
