/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-20
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout.core

/**
 * Identity of the repetitions of a `for` (section 5.7).
 *
 * An element with an `id` inside a `for` is written by the Expander as `id~path`, where the path
 * names the repetitions that contain it, from the outermost to the innermost: `buy~p:7` is the
 * element `buy` of the item 7 of `for p in ...`, and `buy~p:7/o:2` is inside a second `for o`.
 * The program never writes this form: it asks for `buy` and gives the item.
 */
object Scopes {
    /** Separates the id written in the file from the path of the repetition. */
    const val SEP = "~"

    private val KEY_FIELDS = listOf("key", "id")
    private const val HEX = "0123456789ABCDEF"

    /**
     * The automatic key of an item: its field `key`, or else its field `id`.
     * Only text and whole numbers count. Anything else gives null and the position is used.
     */
    fun keyOfItem(item: Any?): String? {
        if (item !is Map<*, *>) return null
        for (field in KEY_FIELDS) {
            when (val v = item[field]) {
                is String -> v.trim().takeIf { it.isNotEmpty() }?.let { return it }
                is Int, is Long, is Short, is Byte -> return v.toString()
                else -> {}
            }
        }
        return null
    }

    /** Letters, digits, `_`, `-` and `.` stay; every other byte becomes `%XX`, so `/`, `:` and `~` never appear. */
    fun encode(s: String): String {
        val sb = StringBuilder(s.length)
        // ALTERAÇÃO AQUI: Usar encodeToByteArray() em vez de toByteArray(Charsets.UTF_8)
        for (b in s.encodeToByteArray()) {
            val c = b.toInt() and 0xFF
            val plain = (c >= 'a'.code && c <= 'z'.code) || (c >= 'A'.code && c <= 'Z'.code) ||
                (c >= '0'.code && c <= '9'.code) || c == '_'.code || c == '-'.code || c == '.'.code
            if (plain) sb.append(c.toChar()) else sb.append('%').append(HEX[c shr 4]).append(HEX[c and 15])
        }
        return sb.toString()
    }

    /** The token of a repetition: its key, or `%p` and the position when there is no key (`%p` never comes out of [encode]). */
    fun token(key: String?, index: Int): String = if (key != null) encode(key) else "%p$index"

    /** The name of the `for` variable of one segment of a path (`p:7` gives `p`). */
    fun variableOf(segment: String): String = segment.substringBefore(':')

    /** The token of one segment of a path (`p:7` gives `7`). */
    fun tokenOf(segment: String): String = segment.substringAfter(':')

    /**
     * True when [given], the item the program holds, is the repetition [held] that the Expander recorded:
     * the same object, an equal one, or one that has the same key.
     */
    fun sameItem(given: Any?, held: Any?, token: String): Boolean {
        if (given === held || given == held) return true
        val key = keyOfItem(given) ?: return false
        return encode(key) == token
    }
}