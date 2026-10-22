/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-22
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout.core

// The core was written for the JVM. These two names exist only on the JVM, so the web build
// declares them here (same package, so they win) and the core stays untouched.

internal inline fun <R> synchronized(@Suppress("UNUSED_PARAMETER") lock: Any, block: () -> R): R = block()

internal object Math {
    fun floor(d: Double): Double = kotlin.math.floor(d)
}

// --- ADIÇÕES PARA O KOTLIN/JS ---

// Simula o java.lang.Character (usado no HtmlParser.kt)
internal object Character {
    fun isValidCodePoint(codePoint: Int): Boolean = codePoint in 0..0x10FFFF
    fun toChars(codePoint: Int): CharArray = charArrayOf(codePoint.toChar())
}

// Simula o java.nio.charset.Charsets (usado no Scopes.kt)
internal object Charsets {
    val UTF_8: dynamic = js("({ name: 'UTF-8' })")
}