/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-15
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout.core

class MarkupError(
    message: String, line: Int, source: String,
    word: String? = null, suggestion: String? = null
) : NativeLayoutError("markup: $message", line, extractLine(source, line), word, suggestion)

sealed class HtmlNode {
    abstract val line: Int
}

private val WHITESPACE = Regex("\\s+")
private val CLASS_CACHE = HashMap<String, List<String>>()

open class HtmlElement(
    val tag: String,
    val attributes: Map<String, String>,
    val children: List<HtmlNode>,
    override val line: Int
) : HtmlNode() {
    internal var chunkText: String? = null

    val id: String? get() = attributes["id"]?.trim()?.takeIf { it.isNotEmpty() }
    val classes: List<String>
        get() {
            val raw = attributes["class"] ?: return emptyList()
            synchronized(CLASS_CACHE) {
                CLASS_CACHE[raw]?.let { return it }
                val parsed = raw.split(WHITESPACE).filter { it.isNotEmpty() }
                if (CLASS_CACHE.size > 1024) CLASS_CACHE.clear()
                CLASS_CACHE[raw] = parsed
                return parsed
            }
        }
}

/** A keyed element whose source text did not change since the last build: nothing was re-parsed. */
class HtmlCachedElement(val chunk: Chunk, line: Int) :
    HtmlElement(chunk.element.tag, chunk.element.attributes, chunk.element.children, line)

class Chunk(
    val text: String,
    val element: HtmlElement,
    val native: Native,
    val parentStyle: NativeStyle,
    val ids: List<Pair<String, Native>>
)

class ChunkCache {
    private val map = HashMap<String, Chunk>()
    private var variables: Map<String, String>? = null

    fun useFor(vars: Map<String, String>) {
        if (variables != vars) {
            map.clear()
            variables = vars
        }
    }

    fun get(key: String): Chunk? = map[key]

    fun put(key: String, chunk: Chunk) {
        if (map.size >= 20000) map.clear()
        map[key] = chunk
    }

    fun clear() = map.clear()
}

class HtmlText(val text: String, override val line: Int) : HtmlNode()

class HtmlParser(
    private val html: String,
    startLine: Int,
    private val source: String,
    private val chunks: ChunkCache? = null
) {
    private var i = 0
    private var line = startLine

    private class Open(val tag: String, val attributes: Map<String, String>, val line: Int, val start: Int) {
        val children = mutableListOf<HtmlNode>()
    }

    private val stack = mutableListOf<Open>()
    private val root = mutableListOf<HtmlNode>()

    private companion object {
        val VOID_TAGS = setOf("br", "hr", "img", "input", "meta", "link")
    }

    private fun err(message: String, at: Int = line, word: String? = null) =
        MarkupError(message, at, source, word)

    private fun target(): MutableList<HtmlNode> = if (stack.isEmpty()) root else stack.last().children

    private fun advanceTo(end: Int) {
        for (k in i until end) if (html[k] == '\n') line++
        i = end
    }

    fun parse(): List<HtmlNode> {
        while (i < html.length) {
            val c = html[i]
            when {
                html.startsWith("<!--", i) -> {
                    val end = html.indexOf("-->", i + 4)
                    if (end < 0) throw err("unclosed HTML comment: missing '-->'")
                    advanceTo(end + 3)
                }
                html.startsWith("<!", i) -> {
                    val end = html.indexOf('>', i)
                    if (end < 0) throw err("unclosed '<!': missing '>'")
                    advanceTo(end + 1)
                }
                html.startsWith("</", i) -> readClose()
                c == '<' && i + 1 < html.length && html[i + 1].isLetter() -> readOpen()
                else -> readText()
            }
        }
        if (stack.isNotEmpty()) {
            val a = stack.last()
            throw err("<${a.tag}> is never closed (opened on line ${a.line})", a.line, "<${a.tag}")
        }
        return root
    }

    private fun readText() {
        var j = i + 1
        while (j < html.length) {
            if (html[j] == '<' && j + 1 < html.length &&
                (html[j + 1].isLetter() || html[j + 1] == '/' || html[j + 1] == '!')
            ) break
            j++
        }
        val raw = html.substring(i, j)
        val l = line
        advanceTo(j)
        if (raw.isNotEmpty()) target().add(HtmlText(decode(raw), l))
    }

    private fun readClose() {
        val tagLine = line
        val end = html.indexOf('>', i)
        if (end < 0) throw err("unclosed '</': missing '>'")
        val tag = html.substring(i + 2, end).trim().lowercase()
        advanceTo(end + 1)
        if (tag in VOID_TAGS) return
        if (stack.isEmpty()) {
            throw err("</$tag> has no matching opening tag", tagLine, "</$tag")
        }
        val top = stack.last()
        if (top.tag != tag) {
            throw err("expected </${top.tag}> (opened on line ${top.line}) before </$tag>", tagLine, "</$tag")
        }
        stack.removeAt(stack.size - 1)
        val element = HtmlElement(top.tag, top.attributes, top.children.toList(), top.line)
        if (chunks != null && keyOf(top.attributes) != null) element.chunkText = html.substring(top.start, end + 1)
        target().add(element)
    }

    private fun keyOf(attributes: Map<String, String>): String? =
        attributes["key"]?.trim()?.takeIf { it.isNotEmpty() }

    private fun readOpen() {
        val tagStart = i
        val tagLine = line
        var j = i + 1
        val nameStart = j
        while (j < html.length && (html[j].isLetterOrDigit() || html[j] == '-')) j++
        val tag = html.substring(nameStart, j).lowercase()
        val attributes = linkedMapOf<String, String>()
        var selfClose = false

        while (true) {
            while (j < html.length && html[j].isWhitespace()) j++
            if (j >= html.length) throw err("unclosed tag <$tag>: missing '>'", tagLine, "<$tag")
            val ch = html[j]
            if (ch == '>') { j++; break }
            if (ch == '/' && j + 1 < html.length && html[j + 1] == '>') { selfClose = true; j += 2; break }
            if (ch == '/') { j++; continue }

            val attrStart = j
            while (j < html.length && !html[j].isWhitespace() && html[j] != '=' && html[j] != '>' && html[j] != '/') j++
            if (j == attrStart) throw err("invalid attribute in tag <$tag>", tagLine, "<$tag")
            val name = html.substring(attrStart, j).lowercase()
            while (j < html.length && html[j].isWhitespace()) j++

            var value = ""
            if (j < html.length && html[j] == '=') {
                j++
                while (j < html.length && html[j].isWhitespace()) j++
                if (j < html.length && (html[j] == '"' || html[j] == '\'')) {
                    val quote = html[j]
                    val end = html.indexOf(quote, j + 1)
                    if (end < 0) throw err("attribute '$name' has an unclosed quote", tagLine, name)
                    value = html.substring(j + 1, end)
                    j = end + 1
                } else {
                    val valStart = j
                    while (j < html.length && !html[j].isWhitespace() && html[j] != '>') j++
                    value = html.substring(valStart, j)
                }
            }
            if (name !in attributes) attributes[name] = decode(value)
        }
        advanceTo(j)

        when {
            tag == "script" ->
                throw err("<script> is not allowed", tagLine, "<script")
            tag == "style" ->
                throw err("<style> is not supported yet; use style=\"...\"", tagLine, "<style")
            tag in VOID_TAGS || selfClose ->
                target().add(HtmlElement(tag, attributes, emptyList(), tagLine))
            else -> {
                val key = if (chunks != null) keyOf(attributes) else null
                val hit = if (key != null) chunks!!.get(tag + "|" + key) else null
                if (hit != null && html.startsWith(hit.text, tagStart)) {
                    advanceTo(tagStart + hit.text.length)
                    target().add(HtmlCachedElement(hit, tagLine))
                } else {
                    stack.add(Open(tag, attributes, tagLine, tagStart))
                }
            }
        }
    }

    private fun decode(t: String): String {
        if ('&' !in t) return t
        val sb = StringBuilder()
        var k = 0
        while (k < t.length) {
            val c = t[k]
            if (c == '&') {
                val end = t.indexOf(';', k)
                if (end in (k + 2)..(k + 10)) {
                    val name = t.substring(k + 1, end)
                    val r: String? = when {
                        name == "amp" -> "&"
                        name == "lt" -> "<"
                        name == "gt" -> ">"
                        name == "quot" -> "\""
                        name == "apos" -> "'"
                        name == "nbsp" -> "\u00A0"
                        name.startsWith("#x") || name.startsWith("#X") ->
                            name.substring(2).toIntOrNull(16)
                                ?.takeIf { Character.isValidCodePoint(it) }
                                ?.let { Character.toChars(it).concatToString() }
                        name.startsWith("#") ->
                            name.substring(1).toIntOrNull()
                                ?.takeIf { Character.isValidCodePoint(it) }
                                ?.let { Character.toChars(it).concatToString() }
                        else -> null
                    }
                    if (r != null) { sb.append(r); k = end + 1; continue }
                }
            }
            sb.append(c)
            k++
        }
        return sb.toString()
    }
}
