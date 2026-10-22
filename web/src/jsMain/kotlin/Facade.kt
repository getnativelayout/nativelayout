/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-22
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

import nativelayout.core.ExpandError
import nativelayout.core.Expander
import nativelayout.core.LayoutEngine
import nativelayout.core.NativeLayoutError
import kotlin.math.abs
import kotlin.math.floor

/**
 * window.nlRender(nl, files, data, dark) -> JSON text.
 * files: JSON object {"name": "text"} with the css and the components; data: JSON text of the Data tab.
 * Success: {"ok":true,"html":"...","warnings":[{"line":1,"message":"..."}]}
 * Failure: {"ok":false,"line":1,"message":"...","context":"...","suggestion":"..."}
 */
fun main() {
    val w: dynamic = js("window")
    w.nlRender = ::render
}

private fun fromJs(v: dynamic): Any? {
    if (v == null) return null
    val t = jsTypeOf(v)
    if (t == "string") return v as String
    if (t == "boolean") return v as Boolean
    if (t == "number") {
        val d = v as Double
        return if (d == floor(d) && abs(d) < 2147483647.0) d.toInt() else d
    }
    if (js("Array.isArray(v)") as Boolean) {
        val n = v.length as Int
        val list = ArrayList<Any?>(n)
        for (i in 0 until n) list.add(fromJs(v[i]))
        return list
    }
    val keys = js("Object.keys(v)")
    val n = keys.length as Int
    val map = LinkedHashMap<String, Any?>()
    for (i in 0 until n) {
        val k = keys[i] as String
        map[k] = fromJs(v[k])
    }
    return map
}

@Suppress("UNCHECKED_CAST")
private fun parseObject(text: String): Map<String, Any?> {
    if (text.isBlank()) return emptyMap()
    val v = fromJs(js("JSON.parse")(text))
    return (v as? Map<String, Any?>) ?: throw IllegalArgumentException("must be a JSON object")
}

private fun q(s: String?): String = if (s == null) "null" else js("JSON.stringify")(s) as String

private fun failure(line: Int, message: String, context: String, suggestion: String?): String =
    "{\"ok\":false,\"line\":$line,\"message\":${q(message)},\"context\":${q(context)},\"suggestion\":${q(suggestion)}}"

fun render(nl: String, files: String, data: String, dark: Boolean): String {
    val fileMap: Map<String, Any?>
    val dataMap: Map<String, Any?>
    try {
        fileMap = parseObject(files)
    } catch (e: Throwable) {
        return failure(1, "Files: " + (e.message ?: "invalid"), "", null)
    }
    try {
        dataMap = parseObject(data)
    } catch (e: Throwable) {
        return failure(1, "Data tab: invalid JSON (" + (e.message ?: "") + ")", "", null)
    }
    var lineMap: List<Int>? = null
    try {
        val expanded = Expander.expand(nl, dataMap, { name -> fileMap[name] as? String }, dark)
        lineMap = expanded.lineMap
        val result = LayoutEngine.build(expanded.html)
        val warnings = result.warnings.joinToString(",") {
            val line = lineMap.getOrNull(it.line - 1) ?: it.line
            "{\"line\":$line,\"message\":${q(it.message)}}"
        }
        return "{\"ok\":true,\"html\":${q(expanded.html)},\"warnings\":[$warnings]}"
    } catch (e: ExpandError) {
        return failure(e.line, e.message, e.context, e.suggestion)
    } catch (e: NativeLayoutError) {
        val line = lineMap?.getOrNull(e.line - 1) ?: e.line
        return failure(line, e.message, e.context, e.suggestion)
    } catch (e: Throwable) {
        return failure(1, e.message ?: "Unexpected error", "", null)
    }
}
