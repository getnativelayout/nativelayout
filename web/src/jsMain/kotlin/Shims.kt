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
